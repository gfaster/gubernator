use std::{ffi::OsStr, os::unix::fs::MetadataExt, path::{Path, PathBuf}};

use super::InstalledPackages;

type PathIndex = std::collections::HashMap<String, PathBuf>;

pub async fn find_packages() -> InstalledPackages {

    // TODO: search for individual managers async
    let res = tokio::task::spawn_blocking(find_packages_blocking).await;

    match res {
        Ok(x) => x,
        Err(e) => {
            eprintln!("{e}");
            InstalledPackages::default()
        },
    }
}

fn find_packages_blocking() -> InstalledPackages {
    let mut ret = InstalledPackages::new();

    // FIXME: this is not even sorta correct
    let exes = index_path();
    let mut add = |name: &str| {
        if exes.contains_key(name) {
            ret.add(name, "UNKNOWN", "0.0.0");
        }
    };

    add("bash");
    add("cargo");
    add("clang");
    add("curl");
    add("find");
    add("gcc");
    add("git");
    add("lscpu");
    add("make");
    add("nebula");
    add("nixos-build-vms");
    add("nix-shell");
    add("ping");
    add("qemu");
    add("qemu-kvm");
    add("rustc");
    add("systemctl");
    add("systemd-nspawn");
    add("systemd-vmspawn");
    add("tar");
    add("xz");
    add("zip");

    ret
}

/// blocking? don't cache. ignores non-utf8 executables
fn index_path() -> PathIndex {
    type InodeSet = std::collections::BTreeSet<u64>;
    let mut seen_inodes = InodeSet::new();

    let mut ret = PathIndex::new();
    let Some(p) = std::env::var_os("PATH") else {
        return ret
    };

    // https://pubs.opengroup.org/onlinepubs/9699919799/basedefs/V1_chap08.html#tag_08
    for path in p.as_encoded_bytes().split(|&b| b == b':') {
        // Safety: since we're splitting on ascii, we can assume each slice is a valid OsStr
        let path = Path::new(unsafe { OsStr::from_encoded_bytes_unchecked(path) });
        if path.as_os_str().is_empty() {
            // POSIX says this is legacy for current dir, I'll ignore it.
            continue
        }

        // skip dirs we've already seen
        // if we fail to stat, it's probably going to fail to read, but I'll stop later since this
        // is just an optimization
        if let Ok(meta) = path.metadata() {
            if !seen_inodes.insert(meta.ino()) {
                continue
            }
        }

        let Ok(dir) = path.read_dir() else { continue };
        for entry in dir {
            let Ok(entry) = entry else { continue };
            let Ok(exe) = entry.file_name().into_string() else { continue };

            // don't overwrite existing
            ret.entry(exe).or_insert_with(|| entry.path());
        }
    }

    todo!()
}
