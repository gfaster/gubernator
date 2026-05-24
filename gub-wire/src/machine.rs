use std::{fmt, num::NonZero, str::FromStr, sync::Arc};

use serde::{Deserialize, Serialize};
use tokio::{sync::{Mutex, MutexGuard}, time::{Duration, Instant}};

use crate::{Memory, uname};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OsRelease {
    pub name: String,
    pub id: String,
    pub pretty_name: String,
    pub version_id: Option<String>,
    pub version: Option<String>,
}

impl OsRelease {
    fn new() -> std::io::Result<Self> {
        // https://www.freedesktop.org/software/systemd/man/latest/os-release.html
        // https://www.linux.org/docs/man5/os-release.html
        let os_rel = match std::fs::read_to_string("/etc/os-release") {
            Ok(x) => Some(x),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                std::fs::read_to_string("/usr/lib/os-release").ok()
            },
            // I'm not sure if it's appropriate to fully error out here
            Err(e) => return Err(e)
        }.unwrap_or_default();

        let lsb_rel = match std::fs::read_to_string("/etc/lsb-release") {
            Ok(x) => Some(x),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                std::fs::read_to_string("/usr/lib/lsb-release").ok()
            },
            // I'm not sure if it's appropriate to fully error out here
            Err(e) => return Err(e)
        }.unwrap_or_default();

        let mut pretty_name = None;
        let mut name = None;
        let mut version_id = None;
        let mut version = None;
        let mut id = None;

        for line in os_rel.lines().chain(lsb_rel.lines()) {
            let line = line.trim();
            if line.starts_with('#') {
                continue
            }
            let Some((key, val)) = line.split_once("=") else {
                continue
            };

            let val = val.trim_start();
            let val = val.trim_matches('"');
            let tgt = match key {
                "PRETTY_NAME" => &mut pretty_name,
                "NAME" => &mut name,
                "VERSION_ID" => &mut version_id,
                "VERSION" => &mut version,
                "ID" => &mut id,
                _ => continue
            };

            // since we also iterate over lsb-release if it exists, we want to whatever comes first
            // to take precidence
            if tgt.is_none() {
                *tgt = Some(val.to_string())
            }
        }

        let default_name = || {
            if cfg!(target_os = "linux") {
                "Linux".into()
            } else if cfg!(target_os = "freebsd") {
                "FreeBSD".into()
            } else {
                // even the freebsd man page defaults cites the above linux.org page as the
                // cannonical spec which says to default to Linux. That doesn't seem like a
                // good idea here.
                todo!("support {} default os-release NAME/PRETTY_NAME", std::env::consts::OS)
            }
        };

        Ok(Self {
            name: name.unwrap_or_else(default_name),
            pretty_name: pretty_name.unwrap_or_else(default_name),
            version_id,
            version,
            id: id.unwrap_or_else(|| std::env::consts::OS.into()),
        })
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Uname {
    pub name: String,
    pub machine: Arch,
    pub release: String,
    pub version: String,
    pub operating_system: String,
    pub nodename: String,
}

impl TryFrom<&uname::Uname> for Uname {
    type Error = ArchParseErr;

    fn try_from(value: &uname::Uname) -> Result<Self, ArchParseErr> {
        Ok(Self {
            name: value.sysname().into(),
            machine: value.machine().parse()?,
            release: value.machine().into(),
            version: value.version().into(),
            operating_system: value.sysname().into(),
            nodename: value.nodename().into()
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename = "lowercase")]
pub enum Arch {
    X86_64,
    Arm64,
}

#[derive(Debug, Clone)]
pub struct ArchParseErr;

impl std::fmt::Display for ArchParseErr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("invalid architecture")
    }
}

impl std::error::Error for ArchParseErr {}

impl FromStr for Arch {
    type Err = ArchParseErr;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "x86_64" | "x64" => Ok(Arch::X86_64),
            "arm64" => Ok(Arch::Arm64),
            _ => Err(ArchParseErr)
        }
    }
}

/// machine status message used for heartbeats and basic usage data
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct MachineStatus {
    pub mem_used: Memory,
    /// availiable cpu as measured tentatively by thread*milliseconds, but I want to eventually
    /// change to something more sophisticated to account for things like cpu speed
    pub avg_avail_cpu: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename = "lowercase")]
pub enum Requirement {
    Require,
    Allow,
    Forbid,
}

impl Requirement {
    pub fn check(self, there: bool) -> bool {
        match self {
            Requirement::Require => there,
            Requirement::Allow => true,
            Requirement::Forbid => !there,
        }
    }
}


/// machine description message
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MachineDesc {
    pub os: OsRelease,
    pub arch: Arch,
    pub ram: Memory,
    pub swap: Memory,
    pub threads: u16,
}





struct MachineDescStateInner {
    last_update: tokio::time::Instant,
    desc: MachineDesc,
    sys: sysinfo::System,
}

/// persistent state for tracking machine status
#[derive(Clone)]
pub struct MachineDescState {
    inner: Arc<Mutex<MachineDescStateInner>>
}

impl MachineDescState {
    fn tracked() ->  sysinfo::RefreshKind {
        sysinfo::RefreshKind::everything().without_processes()
    }

    async fn update(&self) -> MutexGuard<'_, MachineDescStateInner> {
        let mut lock = self.inner.lock().await;
        if lock.last_update.elapsed() >= Duration::from_secs(10) {
            tokio::task::block_in_place(|| lock.sys.refresh_specifics(Self::tracked()));
            lock.last_update = Instant::now();
        }
        lock
    }

    pub async fn wait_for_next_status(&self) -> MachineStatus {
        let wait_for = {
            let mut lock = self.inner.lock().await;
            let now = Instant::now();
            let next_update = lock.last_update + Duration::from_secs(10);
            if next_update <= now {
                self.update_now(&mut lock);
                return self.get_status_of_lock(lock)
            }
            next_update
        };

        tokio::time::sleep_until(wait_for).await;
        self.get_status().await
    }

    fn update_now(&self, inner: &mut MachineDescStateInner) {
        tokio::task::block_in_place(|| inner.sys.refresh_specifics(Self::tracked()));
        inner.last_update = Instant::now();
    }

    fn get_status_of_lock(&self, lock: MutexGuard<MachineDescStateInner>) -> MachineStatus {
        MachineStatus {
            mem_used: Memory::from_bytes(lock.sys.used_memory()),
            avg_avail_cpu: lock.sys.cpus().iter().map(|c| {
                // already in percent, so bringing it up to thousandths
                1000_u64.saturating_sub((c.cpu_usage() * 10.0) as u64)
            }).sum(),
        }
    }

    pub async fn get_status(&self) -> MachineStatus {
        let lock = self.update().await;
        self.get_status_of_lock(lock)
    }

    pub async fn get_desc(&self) -> MachineDesc {
        self.inner.lock().await.desc.clone()
    }

    pub fn new() -> Self {
        let (sys, desc) = tokio::task::block_in_place(|| {
            let sys = sysinfo::System::new_with_specifics(Self::tracked());
            let uname = crate::uname::uname();
            let os = OsRelease::new().unwrap();
            let arch = match uname.machine() {
                "x86_64" => Arch::X86_64,
                a => panic!("unknown arch: {a}")
            };
            let desc = MachineDesc {
                os,
                arch,
                ram: Memory::from_bytes(sys.total_memory()),
                swap: Memory::from_bytes(sys.total_swap()),
                threads: sys.cpus().len().try_into().unwrap_or(u16::MAX),
            };
            (sys, desc)
        });
        Self { inner: Arc::new(Mutex::new(MachineDescStateInner { last_update: Instant::now(), sys, desc })) }
    }
}
