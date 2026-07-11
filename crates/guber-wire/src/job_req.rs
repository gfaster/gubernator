
// See for reference: https://docs.rs/quick-xml/0.39.3/quick_xml/de/index.html

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::sel_expr::MachineSel;

/// Working directory the job will be run in
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkingDir {
    /// Straight up in the home directory. Not recommended if the job does any writes in the cwd.
    Home,
    /// Sets the working directory to root, so most writes will fail
    Root,
    /// Creates a temporary dir that will be cleaned up after exit. For potentially large dirs.
    ManagedTempdir,
    /// Creates a tempdir in [`std::env::temp_dir()`] that will be cleaned up after exit
    Tempdir,
    /// Corresponds to `DynamicUser` setting in `systemd.exec(5)`
    DynamicUser,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Exec {
    pub executable: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub argv0: Option<String>,
    pub argv: Vec<String>,
}

impl Exec {
    pub fn bash_script(script: impl AsRef<str>) -> Self {
        Exec { 
            executable: "/usr/bin/env".into(),
            argv0: None,
            argv: vec!["bash".into(), "-c".into(), script.as_ref().into()]
        }
    }
}

impl fmt::Debug for Exec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if f.alternate() {
            f.debug_struct("Exec").field("executable", &self.executable).field("argv0", &self.argv0).field("argv", &self.argv).finish()
        } else {
            let exe = self.argv0.as_deref().unwrap_or(&self.executable);
            write!(f, "{exe:?}")?;
            for arg in &self.argv {
                write!(f, " {arg:?}")?;
            }
            Ok(())
        }
    }
}

/// Client -> Coordinator
#[derive(Debug, Serialize, Deserialize)]
pub struct JobDescription {
    pub machine_sel: MachineSel,
    pub working_dir: WorkingDir,
    pub exec: Exec
}

/// Coordinator -> Node
#[derive(Debug, Serialize, Deserialize)]
pub struct JobDispatch {
    pub working_dir: WorkingDir,
    pub exec: Exec
}
