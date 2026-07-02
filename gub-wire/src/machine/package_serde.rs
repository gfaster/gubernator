use std::borrow::Cow;

use serde::{Deserialize, Serialize, ser::{SerializeSeq, SerializeStruct}};

use super::InstalledPackages;


type Str<'a> = Cow<'a, str>;

impl<'a> From<PackagesWire<'a>> for InstalledPackages {
    fn from(value: PackagesWire<'a>) -> Self {
        let mut ret = InstalledPackages::new();
        for PackageWire { manager, name, version } in value.package {
            ret.add(name, manager, version);
        }
        ret
    }
}

/// Wire format of package list
#[derive(Debug, Deserialize)]
pub(super) struct PackagesWire<'a> {
    #[serde(default)]
    package: Vec<PackageWire<'a>>,
}

// FIXME: optimization on this is kinda terrible
#[derive(Debug, Serialize, Deserialize)]
struct PackageWire<'a> {
    // field order sorta matters for below tests, but that's it
    name: Str<'a>,
    manager: Str<'a>,
    version: Str<'a>,
}

impl Serialize for InstalledPackages {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer {
        struct Inner<'a>(&'a InstalledPackages);
        impl Serialize for Inner<'_> {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: serde::Serializer {
                let mut sseq = serializer.serialize_seq(Some(self.0.pkgs.len()))?;
                for p in self.0.iter() {
                    let value = PackageWire {
                        manager: Cow::Borrowed(p.manager()),
                        name: Cow::Borrowed(p.name()),
                        version: Cow::Borrowed(p.version()),
                    };
                    sseq.serialize_element(&value)?;
                }
                sseq.end()
            }
        }

        let mut serializer = serializer.serialize_struct("packages", 1)?;
        serializer.serialize_field("package", &Inner(self))?;
        serializer.end()
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    fn test_packages() -> InstalledPackages {
        let mut p = InstalledPackages::new();
        p.add("curl", "apt", "8.2.0");
        p.add("gcc", "apt", "14");
        p.add("rustup", "apt", "1.29.0");
        p.add("rustc", "rustup", "1.96.0");
        p.add("rustc", "rustup", "1.95.0");
        p.add("rustc", "rustup", "1.94.0");
        p
    }

    const TEST_PKG_XML: &str = r###"
    <packages>
        <package> <name>curl</name> <manager>apt</manager> <version>8.2.0</version> </package>
        <package> <name>gcc</name> <manager>apt</manager> <version>14</version> </package>
        <package> <name>rustup</name> <manager>apt</manager> <version>1.29.0</version> </package>
        <package> <name>rustc</name> <manager>rustup</manager> <version>1.96.0</version> </package>
        <package> <name>rustc</name> <manager>rustup</manager> <version>1.95.0</version> </package>
        <package> <name>rustc</name> <manager>rustup</manager> <version>1.94.0</version> </package>
    </packages>
    "###;

    #[test]
    fn serde_round_trip() {
        let pkgs = test_packages();
        let expected: HashSet<_> = pkgs.iter().collect();
        let serialized = quick_xml::se::to_string(&pkgs).unwrap();
        let deserialized: InstalledPackages = quick_xml::de::from_str(&serialized).unwrap();
        let actual: HashSet<_> = deserialized.iter().collect();
        assert_eq!(expected, actual);
    }

    #[test]
    fn empty() {
        let pkgs = InstalledPackages::new();
        let serialized = quick_xml::se::to_string(&pkgs).unwrap();
        let deserialized: InstalledPackages = quick_xml::de::from_str(&serialized).unwrap();
        let actual: HashSet<_> = deserialized.iter().collect();
        assert_eq!(HashSet::new(), actual);
    }

    #[test]
    fn xml_de_format() {
        let pkgs = test_packages();
        let expected: HashSet<_> = pkgs.iter().collect();
        let actual: InstalledPackages = quick_xml::de::from_str(TEST_PKG_XML).unwrap();
        let actual: HashSet<_> = actual.iter().collect();
        assert_eq!(actual, expected)
    }

    #[test]
    fn xml_se_format() {
        let pkgs = test_packages();
        let serialized = quick_xml::se::to_string(&pkgs).unwrap();
        let expected = TEST_PKG_XML.replace([' ', '\n'], "");
        assert_eq!(serialized, expected)
    }

    #[test]
    fn xml_se_format_wrapped() {
        // this test is because I still don't *really* get the semantics here
        #[derive(Serialize)]
        struct Root {
            packages: InstalledPackages,
        }
        let pkgs = Root { packages: test_packages() };
        let serialized = quick_xml::se::to_string(&pkgs).unwrap();
        let expected = TEST_PKG_XML.replace([' ', '\n'], "");
        let expected = format!("<Root>{expected}</Root>");
        assert_eq!(serialized, expected)
    }
}
