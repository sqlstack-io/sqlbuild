//! Search path, environment, provider settings and module identity of a compile.

use sqlbuild_core::process_environment::main::environment_var_os::environment_var_os;
use sqlbuild_core::process_environment::main::environment_vars_os::environment_vars_os;
use std::collections::HashSet;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

use crate::project_reuse::_helpers::files::file_digest;
use crate::project_reuse::constants::NATIVE_DIGEST_PREFIX;
use crate::project_reuse::models::{ReuseRules, SettingsInputs};
use crate::project_snapshot::main::text_path::text_path;

/// Stat each `sys.path` entry, leaving the project root to the project walk.
pub(crate) fn search_path_stamps(
    search_path: &[String],
    project_dir: &Path,
    rules: &ReuseRules,
) -> Vec<(String, i64)> {
    let project_root: Option<PathBuf> = resolved(project_dir);
    search_path
        .iter()
        .map(|entry| {
            let path: PathBuf = if entry.is_empty() {
                PathBuf::from(".")
            } else {
                text_path(entry)
            };
            let mtime_ns: i64 = if project_root.is_some() && resolved(&path) == project_root {
                rules.project_root_path_mtime_ns
            } else {
                match std::fs::metadata(&path) {
                    Ok(metadata) => modified_ns(&metadata),
                    Err(_) => rules.missing_path_mtime_ns,
                }
            };
            (entry.clone(), mtime_ns)
        })
        .collect()
}

fn resolved(path: &Path) -> Option<PathBuf> {
    let Ok(resolved) = std::fs::canonicalize(path) else {
        return None;
    };
    Some(resolved)
}

#[cfg(unix)]
fn modified_ns(metadata: &std::fs::Metadata) -> i64 {
    use std::os::unix::fs::MetadataExt;
    metadata.mtime() * 1_000_000_000 + metadata.mtime_nsec()
}

#[cfg(not(unix))]
fn modified_ns(metadata: &std::fs::Metadata) -> i64 {
    let Ok(modified) = metadata.modified() else {
        return 0;
    };
    match modified.duration_since(std::time::UNIX_EPOCH) {
        Ok(elapsed) => i64::try_from(elapsed.as_nanos()).unwrap_or(i64::MAX),
        Err(_) => 0,
    }
}

#[cfg(unix)]
fn os_bytes(value: &OsString) -> Vec<u8> {
    use std::os::unix::ffi::OsStrExt;
    value.as_bytes().to_vec()
}

#[cfg(not(unix))]
fn os_bytes(value: &OsString) -> Vec<u8> {
    value.to_string_lossy().into_owned().into_bytes()
}

/// Tracked SQLBuild variables (not engine or capture settings) plus template reads, sorted.
pub(crate) fn tracked_environment_names(
    template_names: &[String],
    rules: &ReuseRules,
) -> Vec<String> {
    let mut names: Vec<String> = environment_vars_os()
        .into_iter()
        .filter_map(|(name, _)| unicode_name(name))
        .filter(|name| tracked(name, rules))
        .chain(template_names.iter().cloned())
        .collect::<HashSet<String>>()
        .into_iter()
        .collect();
    names.sort();
    names
}

fn unicode_name(name: OsString) -> Option<String> {
    let Ok(name) = name.into_string() else {
        return None;
    };
    Some(name)
}

fn tracked(name: &str, rules: &ReuseRules) -> bool {
    has_prefix(name, &rules.tracked_prefixes)
        && !rules
            .untracked_names
            .iter()
            .any(|untracked| untracked == name)
}

fn has_prefix(name: &str, prefixes: &[String]) -> bool {
    prefixes
        .iter()
        .any(|prefix| name.starts_with(prefix.as_str()))
}

/// Digest the current values of `names` without retaining them.
pub(crate) fn environment_digest(names: &[String], rules: &ReuseRules) -> String {
    let mut hasher = blake3::Hasher::new();
    for name in names {
        let _ = hasher.update(name.as_bytes());
        let _ = hasher.update(b"\0");
        let value: Vec<u8> = environment_var_os(name).map_or_else(
            || rules.missing_environment_value.as_bytes().to_vec(),
            |value| os_bytes(&value),
        );
        let _ = hasher.update(&value);
        let _ = hasher.update(b"\0");
    }
    format!("{NATIVE_DIGEST_PREFIX}{}", hasher.finalize().to_hex())
}

/// Digest the provider settings values in the environment, env files and secrets directories.
pub(crate) fn settings_inputs_digest(inputs: &[SettingsInputs], rules: &ReuseRules) -> String {
    let mut fields: Vec<Vec<u8>> = Vec::new();
    for item in inputs {
        for (name, value) in settings_environment(item) {
            fields.extend([b"env".to_vec(), name, value]);
        }
        for path in &item.env_files {
            fields.extend([
                b"env_file".to_vec(),
                path.as_bytes().to_vec(),
                contents_digest(Path::new(path), rules).into_bytes(),
            ]);
        }
        for directory in &item.secrets_dirs {
            fields.extend([b"secrets_dir".to_vec(), directory.as_bytes().to_vec()]);
            for (relative, digest) in directory_contents(Path::new(directory), rules) {
                fields.extend([relative.into_bytes(), digest.into_bytes()]);
            }
        }
    }
    let encoded: Vec<u8> = fields.join(&b"\0"[..]);
    format!("{NATIVE_DIGEST_PREFIX}{}", blake3::hash(&encoded).to_hex())
}

fn settings_environment(inputs: &SettingsInputs) -> Vec<(Vec<u8>, Vec<u8>)> {
    let mut matched: Vec<(Vec<u8>, Vec<u8>)> = environment_vars_os()
        .into_iter()
        .filter(|(name, _)| {
            let name: String = name.to_string_lossy().into_owned();
            let key: String = if inputs.case_sensitive {
                name
            } else {
                name.to_lowercase()
            };
            inputs.names.contains(&key) || has_prefix(&key, &inputs.prefixes)
        })
        .map(|(name, value)| (os_bytes(&name), os_bytes(&value)))
        .collect();
    matched.sort();
    matched
}

fn contents_digest(path: &Path, rules: &ReuseRules) -> String {
    file_digest(path).unwrap_or_else(|| rules.missing_file_digest.clone())
}

/// Every file under `directory`, top-down with sorted names, as `os.walk` lists them.
fn directory_contents(directory: &Path, rules: &ReuseRules) -> Vec<(String, String)> {
    let mut contents: Vec<(String, String)> = Vec::new();
    let mut pending: Vec<PathBuf> = vec![directory.to_path_buf()];
    while let Some(root) = pending.pop() {
        let Ok(entries) = std::fs::read_dir(&root) else {
            continue;
        };
        let mut files: Vec<PathBuf> = Vec::new();
        let mut directories: Vec<PathBuf> = Vec::new();
        for entry in entries.flatten() {
            let path: PathBuf = entry.path();
            match std::fs::metadata(&path) {
                Ok(metadata) if metadata.is_dir() => {
                    if !entry.file_type().is_ok_and(|kind| kind.is_symlink()) {
                        directories.push(path);
                    }
                }
                _ => files.push(path),
            }
        }
        files.sort();
        directories.sort();
        for file in files {
            let relative: String = file
                .strip_prefix(directory)
                .map_or_else(|_| file.clone(), Path::to_path_buf)
                .to_string_lossy()
                .into_owned();
            contents.push((relative, contents_digest(&file, rules)));
        }
        pending.extend(directories.into_iter().rev());
    }
    contents
}

/// Stat every listed module file the project walk does not cover, sorted by path.
pub(crate) fn module_stamps(
    module_paths: &[String],
    covered: &HashSet<String>,
) -> Vec<(String, i64, u64)> {
    let mut seen: HashSet<&str> = HashSet::new();
    let mut stamps: Vec<(String, i64, u64)> = Vec::new();
    for path in module_paths {
        if covered.contains(path) || !seen.insert(path.as_str()) {
            continue;
        }
        if let Ok(metadata) = std::fs::metadata(text_path(path)) {
            stamps.push((path.clone(), modified_ns(&metadata), metadata.len()));
        }
    }
    stamps.sort();
    stamps
}

/// Whether every recorded module file still has the same stat identity.
pub(crate) fn module_stamps_unchanged(stamps: &[(String, i64, u64)]) -> bool {
    stamps.iter().all(
        |(path, mtime_ns, size)| match std::fs::metadata(text_path(path)) {
            Ok(metadata) => modified_ns(&metadata) == *mtime_ns && metadata.len() == *size,
            Err(_) => false,
        },
    )
}
