//! Derive a content identity for this native build so local caches notice source changes.
//!
//! The identity hashes every file under the listed entries of every workspace crate plus the
//! workspace manifest (dependency features and build profiles) and lockfile, keyed by
//! `/`-separated paths relative to the crates directory so it matches across platforms. Entries
//! that do not exist (for example the lockfile outside a workspace checkout) are left out of both
//! the hash and the `rerun-if-changed` list, because Cargo reruns a build script on every build
//! when a watched path is missing. The hashed workspace files are exported so tests can assert
//! they are covered.

use sha2::{Digest, Sha256};
use std::path::{Component, Path, PathBuf};

const HASHED_ENTRIES: [&str; 3] = ["Cargo.toml", "src", "function_names"];
const WORKSPACE_FILES: [&str; 2] = ["../../Cargo.toml", "../../Cargo.lock"];

fn main() -> Result<(), String> {
    let crate_dir =
        PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").map_err(|error| error.to_string())?);
    let crates_dir = crate_dir
        .parent()
        .ok_or("native crate has no parent directory")?
        .to_path_buf();
    let mut files: Vec<(String, PathBuf)> = Vec::new();
    for workspace_crate in workspace_crates(&crates_dir)? {
        for entry in HASHED_ENTRIES {
            let path = workspace_crate.join(entry);
            if path.exists() {
                println!("cargo:rerun-if-changed={}", path.display());
                collect_files(&crates_dir, &path, &mut files)?;
            }
        }
    }
    files.sort();
    let mut workspace_files = Vec::new();
    for label in WORKSPACE_FILES {
        let path = crate_dir.join(label);
        if path.is_file() {
            println!("cargo:rerun-if-changed={}", path.display());
            files.push((label.to_owned(), path));
            workspace_files.push(label);
        }
    }
    let mut digest = Sha256::new();
    for (label, file) in &files {
        digest.update(label.as_bytes());
        digest.update([0]);
        digest.update(std::fs::read(file).map_err(|error| error.to_string())?);
        digest.update([0]);
    }
    let identity = format!("{:x}", digest.finalize());
    println!(
        "cargo:rustc-env=SQLBUILD_NATIVE_SOURCE_HASH={}",
        &identity[..16]
    );
    println!(
        "cargo:rustc-env=SQLBUILD_NATIVE_HASHED_WORKSPACE_FILES={}",
        workspace_files.join(",")
    );
    Ok(())
}

/// Every sibling crate directory, so the identity covers the whole native extension.
fn workspace_crates(crates_dir: &Path) -> Result<Vec<PathBuf>, String> {
    let mut crates = Vec::new();
    for entry in std::fs::read_dir(crates_dir).map_err(|error| error.to_string())? {
        let path = entry.map_err(|error| error.to_string())?.path();
        if path.join("Cargo.toml").is_file() {
            crates.push(path);
        }
    }
    crates.sort();
    Ok(crates)
}

fn portable_label(root: &Path, path: &Path) -> Result<String, String> {
    let relative = path.strip_prefix(root).map_err(|error| error.to_string())?;
    let parts: Vec<String> = relative
        .components()
        .map(|component| match component {
            Component::Normal(part) => Ok(part.to_string_lossy().into_owned()),
            other => Err(format!("unexpected path component {other:?}")),
        })
        .collect::<Result<_, _>>()?;
    Ok(parts.join("/"))
}

fn collect_files(
    root: &Path,
    path: &Path,
    files: &mut Vec<(String, PathBuf)>,
) -> Result<(), String> {
    if path.is_file() {
        files.push((portable_label(root, path)?, path.to_path_buf()));
        return Ok(());
    }
    if !path.is_dir() {
        return Ok(());
    }
    for entry in std::fs::read_dir(path).map_err(|error| error.to_string())? {
        collect_files(
            root,
            &entry.map_err(|error| error.to_string())?.path(),
            files,
        )?;
    }
    Ok(())
}
