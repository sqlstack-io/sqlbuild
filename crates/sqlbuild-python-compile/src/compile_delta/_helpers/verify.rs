//! The hidden check that each delta compile matches the full compile it replaces.

use crate::compile_delta::models::Expected;
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, PoisonError};

pub(crate) const MISMATCH_FILE: &str = "delta-mismatch.json";

/// What the full compile produced.
#[derive(Debug, Default)]
pub(crate) struct Actual {
    pub(crate) stdout: Option<String>,
    pub(crate) exit_code: Option<i64>,
    pub(crate) artifacts_written: bool,
    pub(crate) written: HashMap<String, Option<String>>,
}

pub(crate) static EXPECTED: Mutex<Option<Expected>> = Mutex::new(None);
pub(crate) static ACTUAL: Mutex<Option<Actual>> = Mutex::new(None);

pub(crate) fn locked<T>(slot: &Mutex<T>) -> MutexGuard<'_, T> {
    slot.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Every file under `directory`, depth first.
pub(crate) fn files_under(directory: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(directory) else {
        return Vec::new();
    };
    let mut found: Vec<PathBuf> = Vec::new();
    for entry in entries.flatten() {
        let path: PathBuf = entry.path();
        match entry.file_type() {
            Ok(kind) if kind.is_dir() => found.extend(files_under(&path)),
            Ok(kind) if kind.is_file() => found.push(path),
            _ => {}
        }
    }
    found
}

/// The report with its `compile_timings` value blanked.
pub(crate) fn without_timings(stdout: &str) -> String {
    let Some(start) = stdout.find("\n  \"compile_timings\": {") else {
        return stdout.to_owned();
    };
    let Some(length) = stdout[start..].find("\n  },\n") else {
        return stdout.to_owned();
    };
    format!("{}{}", &stdout[..start], &stdout[start + length + 5..])
}

/// Where the files the full compile wrote under `root` differ from what the delta left there.
pub(crate) fn tree_differences(
    (root, expected): (&str, &BTreeMap<String, String>),
    written: &HashMap<String, Option<String>>,
) -> Vec<String> {
    let mut found: Vec<String> = Vec::new();
    let prefix: String = format!("{root}{}", std::path::MAIN_SEPARATOR);
    let full_paths: BTreeMap<&str, &Option<String>> = written
        .iter()
        .filter(|(path, _)| path.starts_with(prefix.as_str()))
        .map(|(path, digest)| (path.as_str(), digest))
        .collect();
    for (path, digest) in &full_paths {
        match (expected.get(*path), digest) {
            (None, _) => found.push(format!("the delta left no artifact at {path}")),
            (Some(left), Some(right)) if left != right => {
                found.push(format!("artifact differs: {path}"));
            }
            _ => {}
        }
    }
    for path in expected.keys() {
        if !full_paths.contains_key(path.as_str()) {
            found.push(format!(
                "the delta left an artifact the full compile does not: {path}"
            ));
        }
    }
    found
}
