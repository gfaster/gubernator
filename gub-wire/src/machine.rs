use std::{fmt, num::NonZero, sync::Arc};

use serde::{Deserialize, Serialize};
use tokio::{sync::{Mutex, MutexGuard}, time::{Duration, Instant}};

use crate::{Memory, uname::Uname};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum Os {
    Linux {
        kernel: (u8, u8, u8),
        has_nix: bool,
        distro: String,
    },
}

impl Os {
    fn current(uname: &Uname) -> Self {
        assert_eq!(uname.sysname(), "Linux");
        let mut it = uname.release().split('.').flat_map(|i| i.parse().ok());
        let kernel = (
            it.next().unwrap_or_default(),
            it.next().unwrap_or_default(),
            it.next().unwrap_or_default(),
        );

        // TODO: fill out these fields
        let distro = String::from("<unknown>");
        Os::Linux { kernel, has_nix: true, distro }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename = "lowercase")]
pub enum Arch {
    X86_64,
    Arm64,
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

/// Represents the minimum needed for a configuration. Can use a slice to represent preference.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MinOs {
    pub linux_kernel: Option<(u8, u8, u8)>,
    pub arch: Arch,
    /// Forbid means no nixos
    pub nix: Requirement,
    pub min_mem: Memory,
    /// additional requirement on top of min_mem
    pub peak_mem: Memory,
    pub peak_mem_can_be_swap: bool,
    pub threads: NonZero<u16>,
    pub avail_cpu: NonZero<u64>,
}

impl MinOs {
    pub fn satisfied(&self, desc: &MachineDesc, status: &MachineStatus) -> bool {
        let MachineDesc {
            ref os,
            arch,
            ram,
            swap,
            threads,
        } = *desc;

        let MachineStatus {
            mem_used,
            avg_avail_cpu,
        } = *status;

        self.arch == arch
            && mem_used + self.min_mem <= ram // can fit min mem in ram
            && if self.peak_mem_can_be_swap {
                mem_used + self.min_mem + self.peak_mem <= ram + swap
            } else {
                mem_used + self.min_mem + self.peak_mem <= ram
            }
            && self.avail_cpu.get() <= avg_avail_cpu
            && self.threads.get() <= threads
            && match os {
                &Os::Linux {
                    kernel,
                    has_nix,
                    distro: _,
                } => {
                    self.linux_kernel.is_none_or(|req_k| req_k <= kernel) && self.nix.check(has_nix)
                }
            }
    }

    pub fn satisfied_reason(&self, desc: &MachineDesc, status: &MachineStatus) -> impl fmt::Display {
        fmt::from_fn(move |f| {
            let MachineDesc {
                ref os,
                arch,
                ram,
                swap,
                threads,
            } = *desc;

            let MachineStatus {
                mem_used,
                avg_avail_cpu,
            } = *status;

            let mut issue_cnt = 0;
            let mut reason_ = |s: fmt::Arguments| {
                if issue_cnt != 0 {
                    f.write_str(" and ")?;
                }
                issue_cnt += 1;
                f.write_fmt(s)
            };

            macro_rules! reason {
                ($($tt:tt)*) => {
                    reason_(format_args!($($tt)*))
                };
            }

            macro_rules! check {
                ($val:expr, $have:expr, $valname:literal $(,)?) => {{
                    let req__: Requirement = $val;
                    let have__: bool = $have;
                    match req__ {
                        Requirement::Require if !have__ => reason!("missing required feature {}", $valname),
                        Requirement::Forbid if have__ => reason!("has disallowed feature {}", $valname),
                        _ => Ok(())
                    }
                }};
            }

            if self.arch != arch {
                return reason!("arch mismatch")
            }


            if mem_used + self.min_mem > ram {
                reason!("can't fit minimum mem ({min:.2?}) in ram ({mem_used:.2?}/{ram:.2?})", min = self.min_mem)?;
            } else {
                let peak = self.peak_mem + self.min_mem;
                if self.peak_mem_can_be_swap && mem_used + self.min_mem + self.peak_mem > ram + swap {
                    reason!("can't fit peak mem ({peak:.2?}) in ram + swap ({mem_used:.2?}/{full:.2?})", full = ram + swap)?;
                } else if mem_used + self.min_mem + self.peak_mem > ram {
                    reason!("can't fit peak mem ({peak:.2?}) in ram ({mem_used:.2?}/{ram:.2?})")?;
                }
            }

            if self.avail_cpu.get() > avg_avail_cpu {
                reason!("want {} cpu timeslices but only {avg_avail_cpu} are availiable", self.avail_cpu)?;
            }

            if self.threads.get() > threads {
                reason!("want {} cpu threads but only {threads} exist", self.threads)?;
            }

            match os {
                &Os::Linux {
                    kernel,
                    has_nix,
                    distro: _,
                } => {

                    if let Some(req_k @ (ra, ri, rp)) = self.linux_kernel && req_k > kernel {
                        let (ha, hi, hp) = kernel;
                        reason!("linux kernel is too old ({ha}.{hi}.{hp} is not at least {ra}.{ri}.{rp})")?;
                    }

                    check!(self.nix, has_nix, "nix packages")?;
                }
            }

            if issue_cnt != 0 {
                return Ok(())
            }
            
            if self.satisfied(desc, status) {
                Ok(())
            } else {
                write!(f, "<BUG> not satisfied but found no reasons")
            }
        })
    }
}

/// machine description message
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MachineDesc {
    pub os: Os,
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
            let os = Os::current(&uname);
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
