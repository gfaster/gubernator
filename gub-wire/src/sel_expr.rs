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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
        match self {
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
                name.as_deref().is_none_or(|_| todo!())
                && machine.is_none_or(|_| todo!())
                && release.as_deref().is_none_or(|_| todo!())
                && version.as_deref().is_none_or(|_| todo!())
                && operating_system.as_deref().is_none_or(|_| todo!())
                && nodename.as_deref().is_none_or(|_| todo!())

            },
            SelExpr::Package { fail: _, name } => todo!(),
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


#[cfg(test)]
mod tests {
    use super::*;

    fn example_machine_sel() -> MachineSel {
        MachineSel {
            predicate: Predicates{ predicate: vec![
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
            ]},
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
        let expr = SelExpr::AnyOf{ 
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
        };
        let s = quick_xml::se::to_string(&expr).unwrap();
        let roundtrip: SelExpr = quick_xml::de::from_str(&s).unwrap();
        assert_eq!(expr, roundtrip)
    }

    #[test]
    fn ser() {
        let ex = example_machine_sel();
        let s = quick_xml::se::to_string(&ex).unwrap();
        println!("{s}");
        let roundtrip: MachineSel = quick_xml::de::from_str(&s).unwrap();
        assert_eq!(ex, roundtrip)
    }
}
