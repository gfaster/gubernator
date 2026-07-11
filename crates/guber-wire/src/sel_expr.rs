use std::fmt;

use serde::{Deserialize, Serialize};

use crate::{Memory, machine::{Arch, MachineDesc}};


#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SelExprFailKind {
    #[default]
    Nothing,
    Explain,
    Warn,
    Ignore,
}

impl SelExprFailKind {
    /// Returns `true` if the sel expr fail kind is [`Nothing`].
    ///
    /// [`Nothing`]: SelExprFailKind::Nothing
    #[must_use]
    pub fn is_nothing(&self) -> bool {
        matches!(self, Self::Nothing)
    }
}

#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SelExpr {
    AnyOf {
        #[serde(rename = "@fail")]
        #[serde(default)]
        #[serde(skip_serializing_if = "SelExprFailKind::is_nothing")]
        fail: SelExprFailKind,

        #[serde(rename = "$value")]
        items: Vec<SelExpr>,
    },
    AllOf {
        #[serde(rename = "@fail")]
        #[serde(default)]
        #[serde(skip_serializing_if = "SelExprFailKind::is_nothing")]
        fail: SelExprFailKind,

        #[serde(rename = "$value")]
        items: Vec<SelExpr>,
    },
    OneOf {
        #[serde(rename = "@fail")]
        #[serde(default)]
        #[serde(skip_serializing_if = "SelExprFailKind::is_nothing")]
        fail: SelExprFailKind,

        #[serde(rename = "$value")]
        items: Vec<SelExpr>,
    },
    Implication {
        #[serde(rename = "@fail")]
        #[serde(default)]
        #[serde(skip_serializing_if = "SelExprFailKind::is_nothing")]
        fail: SelExprFailKind,

        antecedent: Box<SelExpr>,
        consequent: Box<SelExpr>,
    },
    Os {
        #[serde(rename = "@fail")]
        #[serde(default)]
        #[serde(skip_serializing_if = "SelExprFailKind::is_nothing")]
        fail: SelExprFailKind,

        #[serde(rename = "@id")]
        #[serde(skip_serializing_if = "Option::is_none")]
        id: Option<String>,

        #[serde(rename = "@version_id")]
        #[serde(skip_serializing_if = "Option::is_none")]
        version_id: Option<String>,
    },
    Uname {
        #[serde(rename = "@fail")]
        #[serde(default)]
        #[serde(skip_serializing_if = "SelExprFailKind::is_nothing")]
        fail: SelExprFailKind,

        #[serde(rename = "@name")]
        #[serde(skip_serializing_if = "Option::is_none")]
        name: Option<String>,

        #[serde(rename = "@machine")]
        #[serde(skip_serializing_if = "Option::is_none")]
        machine: Option<Arch>,

        #[serde(rename = "@release")]
        #[serde(skip_serializing_if = "Option::is_none")]
        release: Option<String>,

        #[serde(rename = "@version")]
        #[serde(skip_serializing_if = "Option::is_none")]
        version: Option<String>,

        #[serde(rename = "@operating_system")]
        #[serde(skip_serializing_if = "Option::is_none")]
        operating_system: Option<String>,

        #[serde(rename = "@nodename")]
        #[serde(skip_serializing_if = "Option::is_none")]
        nodename: Option<String>,
    },
    Package {
        #[serde(rename = "@fail")]
        #[serde(default)]
        #[serde(skip_serializing_if = "SelExprFailKind::is_nothing")]
        fail: SelExprFailKind,

        #[serde(rename = "$text")]
        name: String,
    },
}

impl SelExpr {
    pub fn fail_kind(&self) -> SelExprFailKind {
        match self {
            SelExpr::AnyOf{ fail, .. } |
            SelExpr::AllOf{ fail, .. } |
            SelExpr::OneOf{ fail, .. } |
            SelExpr::Os { fail, .. } |
            SelExpr::Uname { fail, .. } |
            SelExpr::Implication { fail, .. } |
            SelExpr::Package { fail, .. } => *fail,
        }
    }

    pub fn test_machine(&self, m: &MachineDesc) -> bool {
        // TODO: reports
        let res = match self {
            // connectives
            SelExpr::AnyOf { fail: _, items } => items.iter().any(|i| i.test_machine(m)),
            SelExpr::AllOf { fail: _, items } => items.iter().all(|i| i.test_machine(m)),
            SelExpr::OneOf { fail: _, items } => items.iter().filter(|i| i.test_machine(m)).take(2).count() == 1,
            SelExpr::Implication { fail: _, antecedent, consequent } => !antecedent.test_machine(m) || consequent.test_machine(m),

            SelExpr::Os { fail: _, id, version_id } => {
                id.as_deref().is_none_or(|id| id == m.os.id)
                && version_id.as_deref().is_none_or(|vid| m.os.version_id.as_deref() == Some(vid))
            },
            SelExpr::Uname { fail: _, name, machine, release, version, operating_system, nodename } => {
                macro_rules! uname_field {
                    ($field:ident) => {
                        $field.as_ref().is_none_or(|val| val == &m.uname.$field)
                    };
                }
                name.as_deref().is_none_or(|_| true /* TODO */)
                && machine.is_none_or(|machine| machine == m.uname.machine)
                && uname_field!(release)
                && uname_field!(version)
                && uname_field!(nodename)
                && operating_system.as_deref().is_none_or(|name| name == m.os.name)
            },
            SelExpr::Package { fail: _, name } => {
                // TODO: versioning
                m.packages.get_any_version(name).is_some()
            },
        };

        // eprintln!("RES: {mach} => {self:?}\n\t{res}", mach = m.uname.nodename, res = if res { "PASS" } else { "FAIL" });
        res
    }
}

impl fmt::Debug for SelExpr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AnyOf { fail, items } => {
                let name = match fail {
                    SelExprFailKind::Nothing => "any ",
                    SelExprFailKind::Explain => "any(explain) ",
                    SelExprFailKind::Warn => "any(warn) ",
                    SelExprFailKind::Ignore => "any(ignore) ",
                };
                f.write_str(name)?;
                f.debug_list().entries(items).finish()
            },
            Self::AllOf { fail, items } => {
                let name = match fail {
                    SelExprFailKind::Nothing => "all",
                    SelExprFailKind::Explain => "all(explain)",
                    SelExprFailKind::Warn => "all(warn)",
                    SelExprFailKind::Ignore => "all(ignore)",
                };
                f.write_str(name)?;
                f.debug_list().entries(items).finish()
            },
            Self::OneOf { fail, items } => {
                let name = match fail {
                    SelExprFailKind::Nothing => "one",
                    SelExprFailKind::Explain => "one(explain)",
                    SelExprFailKind::Warn => "one(warn)",
                    SelExprFailKind::Ignore => "one(ignore)",
                };
                f.write_str(name)?;
                f.debug_list().entries(items).finish()
            }
            Self::Implication { fail, antecedent, consequent } => {
                let name = match fail {
                    SelExprFailKind::Nothing => "implication",
                    SelExprFailKind::Explain => "implication(explain)",
                    SelExprFailKind::Warn => "implication(warn)",
                    SelExprFailKind::Ignore => "implication(ignore)",
                };

                f.debug_struct(name).field("antecedent", antecedent).field("consequent", consequent).finish()
            },
            Self::Os { fail, id, version_id } => {
                let name = match fail {
                    SelExprFailKind::Nothing => "os",
                    SelExprFailKind::Explain => "os(explain)",
                    SelExprFailKind::Warn => "os(warn)",
                    SelExprFailKind::Ignore => "os(ignore)",
                };
                let mut d = f.debug_struct(name);
                macro_rules! field {
                    ($id:ident) => {
                        if let Some(x) = $id {
                            d.field(stringify!($id), x);
                        }
                    };
                }
                field!(id);
                field!(version_id);
                d.finish_non_exhaustive()
            },
            Self::Uname { fail, name, machine, release, version, operating_system, nodename } => {
                let sname = match fail {
                    SelExprFailKind::Nothing => "uname",
                    SelExprFailKind::Explain => "uname(explain)",
                    SelExprFailKind::Warn => "uname(warn)",
                    SelExprFailKind::Ignore => "uname(ignore)",
                };
                let mut d = f.debug_struct(sname);
                macro_rules! field {
                    ($id:ident) => {
                        if let Some(x) = $id {
                            d.field(stringify!($id), x);
                        }
                    };
                }
                field!(name);
                field!(machine);
                field!(release);
                field!(version);
                field!(operating_system);
                field!(nodename);

                d.finish_non_exhaustive()
            },
            Self::Package { fail, name } => f.debug_struct("Package").field("fail", fail).field("name", name).finish(),
        }
    }
}

// TODO: validate me!
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MemReq {
    pub min: Memory,
    pub peak: Memory,
    pub swappable: Memory,
}


#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MachineSelElem {
    Predicate(SelExpr),
    Limit(u32),
    Memory(MemReq),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Predicates {
    #[serde(rename="$value")]
    predicate: Vec<SelExpr>
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MachineSel {
    predicate: Predicates,
    limit: Option<u32>,
    memory: MemReq,
}

impl MachineSel {
    /// returns a machine selection that chooses exactly one of a node
    pub fn single_node<I, R>(nodenames: I) -> Self
    where 
        I: IntoIterator<Item = R>,
        R: AsRef<str>
    {
        MachineSel { 
            predicate: Predicates { 
                predicate: [SelExpr::AnyOf { 
                    fail: SelExprFailKind::Nothing,
                    items: Vec::from_iter(nodenames.into_iter().map(|n| {
                        SelExpr::Uname { 
                            fail: SelExprFailKind::Nothing,
                            name: None,
                            machine: None,
                            release: None,
                            version: None,
                            operating_system: None,
                            nodename: Some(n.as_ref().into()) }
                    }))
                }].into()
            },
            limit: Some(1),
            memory: MemReq { min: Memory::from_bytes(0), peak: Memory::from_bytes(0), swappable: Memory::from_bytes(0) }
        }
    }

    /// this will not be final as I can't do reports with this API
    pub fn validate_machines<'a>(&self, i: impl IntoIterator<Item = &'a MachineDesc>) -> impl Iterator<Item = &'a MachineDesc> {
        let mut filter = self.validate_machines_filter();
        i.into_iter().filter(move |m| filter(m))
    }

    /// creates a machine validation filter (it's stateful, so be sure not to call more than once
    /// per job). This will not be final api since I can't do reports.
    pub fn validate_machines_filter(&self) -> impl FnMut(&MachineDesc) -> bool {
        // TODO: memory (and CPU)
        let mut limit = self.limit;
        move |m| {
            if limit == Some(0) {
                return false
            }
            let pass = self.predicate.predicate.iter().all(|p| p.test_machine(m));
            if pass && let Some(limit) = &mut limit {
                *limit -= 1;
            }
            pass
        }
    }
}


#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use crate::machine::InstalledPackages;

    use super::*;

    // TODO: move this to take advantage of it?
    fn example_reimu_desc() -> MachineDesc {
        MachineDesc { 
            packages: Arc::new({
                let mut p = InstalledPackages::new();
                p.add("gcc-14", "", "14.2.0");
                p.add("curl", "", "8.2.0");
                p
            }),
            // TODO; use actual data of reimu for these
            os: crate::machine::OsRelease { 
                name: "Debian".into(),
                id: "debian".into(),
                pretty_name: "Debian".into(),
                version_id: Some("12".into()),
                version: Some("12".into()),
            },
            // TODO; use actual data of reimu for these
            uname: crate::machine::Uname { 
                name: "Linux".into(),
                machine: Arch::X86_64,
                release: "6.12.0".into(),
                version: "SMP PREEMPT_DYNAMIC Mon Jun  1 15:54:55 UTC 2026".into(),
                nodename: "reimu".into(),
            },
            ram: Memory::from_gibibytes(16),
            swap: Memory::from_gibibytes(1),
            threads: 12
        }
    }

    // TODO: move this to take advantage of it?
    fn example_marisa_desc() -> MachineDesc {
        MachineDesc { 
            packages: Arc::new({
                let mut p = InstalledPackages::new();
                p.add("curl", "", "8.2.0");
                p.add("gcc-15", "", "15.2.0");
                p
            }),
            os: crate::machine::OsRelease { 
                name: "NixOS".into(),
                id: "nixos".into(),
                pretty_name: "NixOS 26.11 (Zokor)".into(),
                version_id: Some("26.11".into()),
                version: Some("26.11 (Zokor)".into()),
            },
            uname: crate::machine::Uname { 
                name: "Linux".into(),
                machine: Arch::X86_64,
                release: "7.0.11".into(),
                version: "#1-NixOS SMP PREEMPT_DYNAMIC Mon Jun  1 15:54:55 UTC 2026".into(),
                nodename: "marisa".into(),
            },
            ram: Memory::from_gibibytes(32),
            swap: Memory::from_gibibytes(32),
            threads: 24
        }
    }

    fn example_sel_expr() -> SelExpr {
        SelExpr::AnyOf{ 
            fail: SelExprFailKind::Nothing,
            items: vec![
                SelExpr::AllOf{ 
                    fail: SelExprFailKind::Nothing,
                    items: vec![
                        SelExpr::Os { fail: SelExprFailKind::Nothing, id: Some("debian".into()), version_id: None },
                        SelExpr::Package { fail: SelExprFailKind::Nothing, name: "curl".into()},
                        SelExpr::Package { fail: SelExprFailKind::Nothing, name: "gcc-14".into()},
                    ]
                },
                SelExpr::AllOf{ 
                    fail: SelExprFailKind::Nothing,
                    items: vec![
                        SelExpr::Os { fail: SelExprFailKind::Nothing, id: Some("freebsd".into()), version_id: None },
                        SelExpr::Package { fail: SelExprFailKind::Nothing, name: "curl".into()},
                        SelExpr::Package { fail: SelExprFailKind::Nothing, name: "gcc-14_5".into()},
                    ]
                },
            ]
        }
    }

    fn example_machine_sel() -> MachineSel {
        MachineSel {
            predicate: Predicates{ predicate: vec![ example_sel_expr() ]},
            limit: Some(3),
            memory: MemReq { 
                min: Memory::from_gibibytes(1),
                peak: Memory::from_gibibytes(2),
                swappable: Memory::from_mebibytes(250),
            },
        }
    }

    #[test]
    fn ser_expr() {
        let expr = example_sel_expr();
        let s = quick_xml::se::to_string(&expr).unwrap();
        let roundtrip: SelExpr = quick_xml::de::from_str(&s).unwrap();
        assert_eq!(expr, roundtrip)
    }

    #[test]
    fn ser() {
        let ex = example_machine_sel();
        let s = quick_xml::se::to_string(&ex).unwrap();
        // println!("{s}");
        let roundtrip: MachineSel = quick_xml::de::from_str(&s).unwrap();
        assert_eq!(ex, roundtrip)
    }

    #[test]
    fn test_machine() {
        let reimu = example_reimu_desc();
        let marisa = example_marisa_desc();

        let case = |expected: &[&str], sel: &MachineSel| {
            let actual: Vec<_> = sel.validate_machines([&reimu, &marisa]).map(|m| &m.uname.nodename).collect();
            // if this fails because of ordering, just swap it. not a bug.
            assert_eq!(actual, expected)
        };

        let sel = example_machine_sel();

        case(&["reimu"], &sel);
        
    }
}
