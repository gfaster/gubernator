use std::{collections::HashMap, fmt, hash::Hash, str::FromStr, sync::Arc};

use serde::{Deserialize, Serialize};
use tokio::{sync::{Mutex, MutexGuard}, time::{Duration, Instant}};

use crate::{Memory, uname};

mod find_packages;
mod package_serde;

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
    pub nodename: String,
}

impl TryFrom<&uname::Uname> for Uname {
    type Error = ArchParseErr;

    fn try_from(value: &uname::Uname) -> Result<Self, ArchParseErr> {
        Ok(Self {
            name: value.sysname().into(),
            machine: value.machine().parse()?,
            release: value.release().into(),
            version: value.version().into(),
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

impl fmt::Display for ArchParseErr {
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
    pub packages: Arc<InstalledPackages>,
    pub os: OsRelease,
    pub uname: Uname,
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

    pub async fn new() -> Self {
        let packages = Arc::new(find_packages::find_packages().await);
        let (sys, desc) = {
            let sys = sysinfo::System::new_with_specifics(Self::tracked());
            let uname = crate::uname::uname();
            let os = OsRelease::new().unwrap();
            let uname = (&uname).try_into().unwrap();
            let desc = MachineDesc {
                packages,
                os,
                uname,
                ram: Memory::from_bytes(sys.total_memory()),
                swap: Memory::from_bytes(sys.total_swap()),
                threads: sys.cpus().len().try_into().unwrap_or(u16::MAX),
            };
            (sys, desc)
        };
        Self { inner: Arc::new(Mutex::new(MachineDescStateInner { last_update: Instant::now(), sys, desc })) }
    }
}



#[derive(Default, Deserialize)]
#[serde(from = "package_serde::PackagesWire")]
pub struct InstalledPackages {
    // TODO: replace internals with hashbrown hashtables to reduce duplicate allocs

    managers: Vec<String>,
    pkgs: Vec<PackageInner>,
    pkgs_by_name: HashMap<String, Vec<usize>>,

    /// first element of tuple is index into `managers`
    pkgs_by_manager: HashMap<String, (usize, Vec<usize>)>,
}

impl InstalledPackages {
    pub fn new() -> Self {
        Self::default()
    }

    fn _add<'a>(&'a mut self, name: String, manager: &str, version: String) -> Package<'a> {
        let pkg_idx = self.pkgs.len();
        let manager_idx = 'mgr: {
            // package managers don't get added too often, so don't use entry api
            if let Some(&mut (mgr_idx, ref mut mgr)) = self.pkgs_by_manager.get_mut(manager) {
                mgr.push(pkg_idx);
                break 'mgr mgr_idx
            }

            let mgr_idx = self.managers.len();
            self.managers.push(manager.into());
            self.pkgs_by_manager.insert(manager.into(), (mgr_idx, vec![pkg_idx]));
            mgr_idx
        };

        self.pkgs_by_name.entry(name.clone()).or_default().push(pkg_idx);

        self.pkgs.push(PackageInner { manager_idx, name, version });
        Package { container: self, inner: &self.pkgs[pkg_idx] }
    }

    pub fn iter(&self) -> impl Iterator<Item = Package<'_>> {
        self.pkgs.iter().map(|inner| Package { container: self, inner })
    }

    pub fn add<'a>(&'a mut self, name: impl AsRef<str>, manager: impl AsRef<str>, version: impl AsRef<str>) -> Package<'a> {
        // do as_ref().into() to try to help the optimizer eliminate string allocations while
        // keeping a nice UI
        self._add(name.as_ref().into(), manager.as_ref(), version.as_ref().into())
    }

    fn get_inner<'a>(&'a self, name: &str, version_test: impl Fn(&str) -> bool) -> Option<Package<'a>> {
        let versions = self.pkgs_by_name.get(name)?;
        let idx = versions.iter().copied().find(|&i| version_test(&self.pkgs[i].version))?;
        Some(Package { container: self, inner: &self.pkgs[idx] })
    }

    pub fn get_any_version(&self, name: impl AsRef<str>) -> Option<Package<'_>> {
        self.get_inner(name.as_ref(), |_| true)
    }

    pub fn get(&self, name: impl AsRef<str>, version: impl AsRef<str>) -> Option<Package<'_>> {
        let version = version.as_ref();
        self.get_inner(name.as_ref(), |v| v == version)
    }
}

struct PackageInner {
    manager_idx: usize,
    name: String,
    version: String,
}

pub struct Package<'a> {
    container: &'a InstalledPackages,
    inner: &'a PackageInner,
}

impl<'a> Package<'a> {
    pub fn manager(&self) -> &'a str {
        &self.container.managers[self.inner.manager_idx]
    }

    pub fn name(&self) -> &'a str {
        &self.inner.name
    }

    pub fn version(&self) -> &'a str {
        &self.inner.version
    }

    /// for equality and hashing
    fn fields(&self) -> [&'a str; 3] {
        // roughly ordered by likelihood different
        [
            self.name(),
            self.version(),
            self.manager(),
        ]
    }
}

impl Hash for Package<'_> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.fields().hash(state);
    }
}

impl PartialEq for Package<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.fields() == other.fields()
    }
}

impl Eq for Package<'_> {}

impl fmt::Debug for Package<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Package")
            .field("name", &self.name())
            .field("version", &self.version())
            .field("manager", &self.manager())
            .finish()
    }
}

impl fmt::Debug for InstalledPackages {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Packages")
            .field("managers", &self.managers)
            .field("pkgs", &fmt::from_fn(|f| f.debug_list().entries(self.iter()).finish()))
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn smoke() {
        let mut pkgs = InstalledPackages::new();
        pkgs.add("curl", "dpkg", "8.2.0");
        pkgs.add("gcc-15", "dpkg", "15.2.0");

        let curl = pkgs.get_any_version("curl").unwrap();
        assert_eq!(curl.name(), "curl");
        assert_eq!(curl.version(), "8.2.0");

        let gcc = pkgs.get_any_version("gcc-15").unwrap();
        assert_eq!(gcc.name(), "gcc-15");
        assert_eq!(gcc.version(), "15.2.0");
    }
}
