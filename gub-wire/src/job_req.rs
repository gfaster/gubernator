
// See for reference: https://docs.rs/quick-xml/0.39.3/quick_xml/de/index.html

use serde::{Deserialize, Serialize};

use crate::machine::MinOs;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PackageInclusion {
    Required,
    Forbidden,
}

/// satisfaction has to be determined on a per-package manager basis
#[derive(Debug, Serialize, Deserialize)]
pub struct PackageVersion {
    pub not_older_than: Option<String>,
    pub not_newer_than: Option<String>,
}

/// Dependency describing a single package. Very important to me that this remains purely
/// functional.
#[derive(Debug, Serialize, Deserialize)]
pub struct PackageDependency {
    /// The package manager this package is installed under. It has to be known by the node.
    pub manager: String,

    /// The name of the package to use. I have not decided on how to handle special/custom packages
    /// such as from Nix
    pub name: String,

    #[serde(rename = "$value")]
    pub inclusion: PackageInclusion,

    pub version_satisfies_any_of: Vec<PackageVersion>,
}

/// config of one job variant 
#[derive(Debug, Serialize, Deserialize)]
pub struct JobConfiguration {
    pub min_os: MinOs,
}


pub struct JobRequest {

}
