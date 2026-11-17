//! The hidden check that each delta compile matches the full compile it replaces.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, PoisonError};

use pyo3::prelude::{Bound, PyAny, PyResult};
use sqlbuild_cache::digest::main::digest_files::digest_files;
use sqlbuild_cache::digest::main::hex_digest::hex_digest;
use sqlbuild_cache::project_reuse::constants::NATIVE_DIGEST_PREFIX;

use crate::bindings::_helpers::compile_command::reuse_recording::CompileArtifactWrites;
use crate::bindings::_helpers::compile_command::reuse_recording::RecordedCompileOutput;

/// The exit code of a compile whose delta differed from its full compile.
pub(crate) const MISMATCH_EXIT_CODE: i64 = 70;
const MISMATCH_FILE: &str = "delta-mismatch.json";

/// What the delta compile produced, held until the full compile finishes.
#[derive(Debug, Default)]
pub(crate) struct Expected {
    pub(crate) stdout: Option<String>,
    pub(crate) exit_code: Option<i64>,
    pub(crate) raised: Option<String>,
    /// Every file under the compiled root after the delta, with its native digest.
    pub(crate) compiled: BTreeMap<String, String>,
    pub(crate) compiled_root: String,
    pub(crate) artifacts_written: bool,
    /// Every file under `target/compiled-failed/` after the delta, with its native digest.
    pub(crate) failed: BTreeMap<String, String>,
    pub(crate) failed_root: String,
}

/// What the full compile produced.
#[derive(Debug, Default)]
struct Actual {
    stdout: Option<String>,
    exit_code: Option<i64>,
    artifacts_written: bool,
    written: HashMap<String, Option<String>>,
}

static EXPECTED: Mutex<Option<Expected>> = Mutex::new(None);
static ACTUAL: Mutex<Option<Actual>> = Mutex::new(None);

fn locked<T>(slot: &Mutex<T>) -> MutexGuard<'_, T> {
    slot.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Every file under `directory`, depth first.
fn files_under(directory: &Path) -> Vec<PathBuf> {
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

/// Every file under `root` with its native digest, keyed by absolute path text.
pub(crate) fn compiled_digests(root: &Path) -> BTreeMap<String, String> {
    let paths: Vec<PathBuf> = files_under(root);
    let digests = digest_files(&paths);
    paths
        .into_iter()
        .zip(digests)
        .filter_map(|(path, digest)| {
            let Ok(digest) = digest else {
                return None;
            };
            Some((
                path.to_string_lossy().into_owned(),
                format!("{NATIVE_DIGEST_PREFIX}{}", hex_digest(&digest)),
            ))
        })
        .collect()
}

/// Hold the delta's result for comparison once the full compile finishes.
pub(crate) fn hold_expected(expected: Expected) {
    *locked(&EXPECTED) = Some(expected);
    *locked(&ACTUAL) = None;
}

/// Note the full compile's report, exit code and artifact writes, when a check is armed.
pub(crate) fn full_compile_finished(
    output: &Bound<'_, PyAny>,
    (exit_code, artifacts_written): (i64, bool),
    artifact_writes: &Bound<'_, PyAny>,
) -> PyResult<()> {
    if locked(&EXPECTED).is_none() {
        return Ok(());
    }
    let (_, stdout): (Vec<String>, Option<String>) = output
        .cast::<RecordedCompileOutput>()?
        .get()
        .texts(output.py())?;
    let written: HashMap<String, Option<String>> = artifact_writes
        .cast::<CompileArtifactWrites>()
        .map(|writes| writes.get().written_digests())
        .unwrap_or_default();
    *locked(&ACTUAL) = Some(Actual {
        stdout,
        exit_code: Some(exit_code),
        artifacts_written,
        written,
    });
    Ok(())
}

/// The report with its `compile_timings` value blanked.
fn without_timings(stdout: &str) -> String {
    let Some(start) = stdout.find("\n  \"compile_timings\": {") else {
        return stdout.to_owned();
    };
    let Some(length) = stdout[start..].find("\n  },\n") else {
        return stdout.to_owned();
    };
    format!("{}{}", &stdout[..start], &stdout[start + length + 5..])
}

/// Compare the held delta result with the full compile's; every difference found.
pub(crate) fn differences(raised: Option<String>) -> Vec<String> {
    let Some(expected) = locked(&EXPECTED).take() else {
        return Vec::new();
    };
    let actual: Actual = locked(&ACTUAL).take().unwrap_or_default();
    let mut found: Vec<String> = Vec::new();
    if expected.raised != raised {
        found.push(format!(
            "raised error differs: delta {:?}, full {:?}",
            expected.raised, raised
        ));
    }
    if expected.raised.is_some() || raised.is_some() {
        return found;
    }
    if expected.exit_code != actual.exit_code {
        found.push(format!(
            "exit code differs: delta {:?}, full {:?}",
            expected.exit_code, actual.exit_code
        ));
    }
    let delta_stdout: Option<String> = expected.stdout.as_deref().map(without_timings);
    let full_stdout: Option<String> = actual.stdout.as_deref().map(without_timings);
    if delta_stdout != full_stdout {
        let (delta, full) = (
            delta_stdout.unwrap_or_default(),
            full_stdout.unwrap_or_default(),
        );
        let common: usize = delta
            .bytes()
            .zip(full.bytes())
            .take_while(|(left, right)| left == right)
            .count();
        let start: usize = common.saturating_sub(200);
        let excerpt = |text: &str| -> String {
            text.get(start..(common + 300).min(text.len()))
                .unwrap_or_default()
                .to_owned()
        };
        found.push(format!(
            "report differs at byte {common}:\n--- delta\n{}\n--- full\n{}",
            excerpt(&delta),
            excerpt(&full)
        ));
    }
    if expected.artifacts_written != actual.artifacts_written {
        found.push(format!(
            "artifact writing differs: delta {}, full {}",
            expected.artifacts_written, actual.artifacts_written
        ));
    }
    found.extend(tree_differences(
        (&expected.failed_root, &expected.failed),
        &actual.written,
    ));
    if actual.artifacts_written {
        found.extend(tree_differences(
            (&expected.compiled_root, &expected.compiled),
            &actual.written,
        ));
    }
    found.truncate(50);
    found
}

/// Where the files the full compile wrote under `root` differ from what the delta left there.
fn tree_differences(
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

/// Write the differences beside the compiler cache for inspection.
pub(crate) fn write_mismatch(cache_dir: &Path, differences: &[String]) {
    let _ = std::fs::create_dir_all(cache_dir);
    let text: String = serde_json::to_string_pretty(differences).unwrap_or_default();
    let _ = std::fs::write(cache_dir.join(MISMATCH_FILE), text);
}

/// Forget any held result.
pub(crate) fn disarm() {
    *locked(&EXPECTED) = None;
    *locked(&ACTUAL) = None;
}
