//! Whole-project compile reuse on the shared native store, driven by the Python host's facts.

use std::collections::{BTreeMap, HashMap};
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use pyo3::exceptions::PyValueError;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult, Python};
use pyo3::{FromPyObject, IntoPyObject, pyclass, pyfunction, pymethods, wrap_pyfunction};
use sqlbuild_cache::digest::main::bytes_digest::bytes_digest;
use sqlbuild_cache::digest::main::hex_digest::hex_digest;
use sqlbuild_cache::project_reuse::constants::{NATIVE_DIGEST_PREFIX, REUSE_STORE_SUFFIX};
use sqlbuild_cache::project_reuse::main::check_reuse::check_reuse;
use sqlbuild_cache::project_reuse::main::pending_digest_bytes::pending_digest_bytes;
use sqlbuild_cache::project_reuse::main::record_reuse::record_reuse;
use sqlbuild_cache::project_reuse::main::replay_stdout::replay_stdout;
use sqlbuild_cache::project_reuse::models::{
    CompileRecord, HostIdentity, RecordOutcome, RecordResult, RecordedArtifact, ReuseAttempt,
    ReuseCheck, ReuseOutcome, ReuseRules, SettingsInputs, StoredOutput,
};
use sqlbuild_cache::project_snapshot::main::path_text::path_text;
use sqlbuild_cache::project_snapshot::models::SnapshotRules;

use crate::bindings::_helpers::boundary::panics::compiler_guard;

positional_record! {
    extract;
    /// Settings case sensitivity, names, prefixes, env files and secrets directories.
    pub(crate) struct SettingsRow {
        pub(crate) case_sensitive: bool,
        pub(crate) names: Vec<String>,
        pub(crate) prefixes: Vec<String>,
        pub(crate) env_files: Vec<String>,
        pub(crate) secrets_dirs: Vec<String>,
    }
}
type ReplayRow = (String, Vec<String>, i64);
type LaunchIdentityRow = (String, BTreeMap<String, String>, Vec<String>);
/// A written artifact's digest, or a kept artifact's size and mtime.
pub(crate) type ArtifactRow = (Option<String>, Option<u64>, Option<i64>);

/// The host constants the walk, environment and store checks use.
#[derive(FromPyObject, IntoPyObject)]
#[pyo3(from_item_all)]
pub(crate) struct ReuseRulesPy {
    pub(crate) excluded: Vec<String>,
    pub(crate) excluded_root: Vec<String>,
    pub(crate) presence_suffixes: Vec<String>,
    pub(crate) output_files: Vec<(u64, u64)>,
    pub(crate) tracked_prefixes: Vec<String>,
    pub(crate) untracked_names: Vec<String>,
    pub(crate) disabling_variables: Vec<(String, String)>,
    pub(crate) config_filenames: Vec<String>,
    pub(crate) racy_window_ns: i64,
    pub(crate) max_stored_entries: usize,
    pub(crate) max_entry_bytes: usize,
    pub(crate) missing_environment_value: String,
    pub(crate) missing_file_digest: String,
    pub(crate) missing_path_mtime_ns: i64,
    pub(crate) project_root_path_mtime_ns: i64,
    pub(crate) compiled_root: PathBuf,
    pub(crate) retired_directories: Vec<String>,
}

impl From<ReuseRulesPy> for ReuseRules {
    fn from(rules: ReuseRulesPy) -> Self {
        Self {
            snapshot: SnapshotRules {
                excluded: rules.excluded,
                excluded_root: rules.excluded_root,
                presence_suffixes: rules.presence_suffixes,
                output_files: rules.output_files,
            },
            tracked_prefixes: rules.tracked_prefixes,
            untracked_names: rules.untracked_names,
            disabling_variables: rules.disabling_variables,
            config_filenames: rules.config_filenames,
            racy_window_ns: rules.racy_window_ns,
            max_stored_entries: rules.max_stored_entries,
            max_entry_bytes: rules.max_entry_bytes,
            missing_environment_value: rules.missing_environment_value,
            missing_file_digest: rules.missing_file_digest,
            missing_path_mtime_ns: rules.missing_path_mtime_ns,
            project_root_path_mtime_ns: rules.project_root_path_mtime_ns,
            compiled_root: rules.compiled_root,
            retired_directories: rules.retired_directories,
        }
    }
}

/// One reuse check: where the project and its store slot are, and the host's identity facts.
#[derive(FromPyObject)]
#[pyo3(from_item_all)]
pub(crate) struct ReuseCheckPy {
    pub(crate) project_dir: PathBuf,
    pub(crate) store_directory: PathBuf,
    pub(crate) selected_target: Option<OsString>,
    pub(crate) invocation: Vec<u8>,
    pub(crate) runtime: BTreeMap<String, OsString>,
    pub(crate) search_path: Vec<OsString>,
    pub(crate) bypass_requested: bool,
    pub(crate) json_output: bool,
    pub(crate) rules: ReuseRulesPy,
}

/// What a full compile leaves for recording; `artifacts` maps a path to `(digest, size, mtime)`.
#[derive(FromPyObject)]
#[pyo3(from_item_all)]
pub(crate) struct CompileRecordPy {
    pub(crate) stdout: Option<String>,
    pub(crate) stderr_lines: Vec<String>,
    pub(crate) exit_code: i64,
    pub(crate) compile_cache_enabled: bool,
    pub(crate) read_run_id: bool,
    pub(crate) settings_inputs: Option<Vec<SettingsRow>>,
    pub(crate) template_environment_names: Vec<String>,
    pub(crate) module_paths: Vec<OsString>,
    pub(crate) artifacts: HashMap<String, ArtifactRow>,
    pub(crate) artifacts_written: bool,
    pub(crate) dag_path: Option<String>,
    pub(crate) json_output: bool,
    pub(crate) rules: ReuseRulesPy,
}

/// One invocation's native reuse state, from the check to the record.
#[pyclass(module = "sqlbuild._native", frozen)]
pub(crate) struct NativeReuseAttempt {
    attempt: Mutex<ReuseAttempt>,
}

fn poisoned<T>(_: T) -> pyo3::PyErr {
    PyValueError::new_err("compile reuse attempt is poisoned")
}

impl NativeReuseAttempt {
    /// Run `body` on the native attempt.
    pub(crate) fn with_attempt<T>(&self, body: impl FnOnce(&ReuseAttempt) -> T) -> PyResult<T> {
        let attempt = self.attempt.lock().map_err(poisoned)?;
        Ok(body(&attempt))
    }
}

#[pymethods]
impl NativeReuseAttempt {
    /// `"hit"`, `"miss"` or `"bypass"`.
    fn outcome(&self) -> PyResult<&'static str> {
        Ok(match self.attempt.lock().map_err(poisoned)?.outcome {
            ReuseOutcome::Hit => "hit",
            ReuseOutcome::Miss => "miss",
            ReuseOutcome::Bypass => "bypass",
        })
    }

    /// The store file holding this invocation's slot.
    fn store_path(&self) -> PyResult<String> {
        let attempt = self.attempt.lock().map_err(poisoned)?;
        Ok(attempt.store_path.to_string_lossy().into_owned())
    }

    /// Whether a stored compile matched but could not be replayed.
    fn replay_failed(&self) -> PyResult<bool> {
        Ok(self.attempt.lock().map_err(poisoned)?.replay_failed)
    }

    /// How many project paths the check stamped.
    fn snapshot_paths(&self) -> PyResult<usize> {
        Ok(self.attempt.lock().map_err(poisoned)?.snapshot.len())
    }

    /// Bytes recording would hash for files whose stamp moved.
    fn pending_digest_bytes(&self) -> PyResult<u64> {
        let attempt = self.attempt.lock().map_err(poisoned)?;
        Ok(pending_digest_bytes(&attempt))
    }

    /// On a hit, the stored report with these timings plus its stderr lines and exit code.
    fn replay(&self, timings: Vec<(String, i64)>) -> PyResult<Option<ReplayRow>> {
        let attempt = self.attempt.lock().map_err(poisoned)?;
        let Some(output) = attempt.replay.as_ref() else {
            return Ok(None);
        };
        let output: &StoredOutput = output;
        Ok(replay_stdout(output, &timings)
            .map(|stdout| (stdout, output.stderr_lines.clone(), output.exit_code)))
    }

    /// The invocation digest, runtime and search path the check compared, for a launch record.
    fn launch_identity(&self) -> PyResult<LaunchIdentityRow> {
        let attempt = self.attempt.lock().map_err(poisoned)?;
        let identity: &HostIdentity = &attempt.identity;
        Ok((
            identity.invocation_digest.clone(),
            identity.runtime.clone(),
            identity.search_path.clone(),
        ))
    }

    /// How many project files the check read for content digests.
    fn digested_files(&self) -> PyResult<usize> {
        Ok(self.attempt.lock().map_err(poisoned)?.digested_files)
    }

    /// Store this compile or clear the slot; a failure clears the slot and raises `OSError`.
    pub(crate) fn record(
        &self,
        py: Python<'_>,
        record: CompileRecordPy,
    ) -> PyResult<(&'static str, usize)> {
        compiler_guard(|| {
            let rules: ReuseRules = record.rules.into();
            let mut compile: CompileRecord = CompileRecord {
                stdout: record.stdout,
                stderr_lines: record.stderr_lines,
                exit_code: record.exit_code,
                compile_cache_enabled: record.compile_cache_enabled,
                read_run_id: record.read_run_id,
                settings: record
                    .settings_inputs
                    .map(|rows| rows.into_iter().map(settings_inputs).collect())
                    .ok_or_else(String::new),
                template_environment_names: record.template_environment_names,
                module_paths: texts(&record.module_paths),
                artifacts: record
                    .artifacts
                    .into_iter()
                    .map(|(path, artifact)| (path, recorded_artifact(artifact)))
                    .collect(),
                artifacts_written: record.artifacts_written,
                dag_path: record.dag_path,
                json_output: record.json_output,
                image: crate::bindings::_helpers::compile_context::delta::take_pending_image(),
                vouched_target_stamps: None,
            };
            if let Some(merge) =
                crate::bindings::_helpers::compile_context::delta::take_record_merge()
            {
                for stamp in &merge.kept {
                    compile
                        .artifacts
                        .entry(stamp.relative_path.clone())
                        .or_insert(RecordedArtifact::Kept {
                            size: stamp.size,
                            mtime_ns: stamp.mtime_ns,
                        });
                }
                compile.vouched_target_stamps = Some(merge.kept);
                compile
                    .template_environment_names
                    .extend(merge.environment_names);
                compile.template_environment_names.sort();
                compile.template_environment_names.dedup();
                compile.module_paths.extend(merge.module_paths);
            }
            let attempt = self.attempt.lock().map_err(poisoned)?;
            let result: RecordResult = py
                .detach(|| record_reuse(&attempt, &compile, &rules))
                .map_err(|error| pyo3::exceptions::PyOSError::new_err(error.message))?;
            let outcome: &'static str = match result.outcome {
                RecordOutcome::NotAttempted => "not_attempted",
                RecordOutcome::Stored => "stored",
                RecordOutcome::Removed => "removed",
                RecordOutcome::Skipped => "skipped",
            };
            Ok((outcome, result.digested_files))
        })
    }
}

fn settings_inputs(row: SettingsRow) -> SettingsInputs {
    let SettingsRow {
        case_sensitive,
        names,
        prefixes,
        env_files,
        secrets_dirs,
    } = row;
    SettingsInputs {
        case_sensitive,
        names,
        prefixes,
        env_files,
        secrets_dirs,
    }
}

fn recorded_artifact(artifact: ArtifactRow) -> RecordedArtifact {
    match artifact {
        (Some(digest), _, _) => RecordedArtifact::Written { digest },
        (None, size, mtime_ns) => RecordedArtifact::Kept {
            size: size.unwrap_or_default(),
            mtime_ns: mtime_ns.unwrap_or_default(),
        },
    }
}

/// Check the stored compile for this invocation, replaying it on a hit.
#[pyfunction]
pub(crate) fn check_compile_reuse(
    py: Python<'_>,
    check: ReuseCheckPy,
) -> PyResult<NativeReuseAttempt> {
    compiler_guard(|| {
        let rules: ReuseRules = check.rules.into();
        let request: ReuseCheck<'_> = ReuseCheck {
            project_dir: check.project_dir,
            store_path: store_slot(&check.store_directory, check.selected_target.as_deref()),
            identity: HostIdentity {
                invocation_digest: native_digest(&check.invocation),
                runtime: check
                    .runtime
                    .iter()
                    .map(|(name, value)| (name.clone(), path_text(value)))
                    .collect(),
                search_path: texts(&check.search_path),
            },
            bypass_requested: check.bypass_requested,
            json_output: check.json_output,
            rules: &rules,
        };
        let attempt: ReuseAttempt = py.detach(|| check_reuse(request));
        Ok(NativeReuseAttempt {
            attempt: Mutex::new(attempt),
        })
    })
}

/// The native content digest of artifact bytes, as reuse verifies written artifacts.
pub(crate) fn artifact_digest(contents: &[u8]) -> String {
    native_digest(contents)
}

fn native_digest(contents: &[u8]) -> String {
    format!(
        "{NATIVE_DIGEST_PREFIX}{}",
        hex_digest(&bytes_digest(contents))
    )
}

/// The one store file kept for the selected target.
fn store_slot(directory: &Path, selected_target: Option<&OsStr>) -> PathBuf {
    let label: String = selected_target.map_or_else(String::new, |target| {
        format!("target:{}", path_text(target))
    });
    directory.join(format!(
        "{}.{REUSE_STORE_SUFFIX}",
        hex_digest(&bytes_digest(label.as_bytes()))
    ))
}

/// Host strings that may carry undecodable bytes, as exact text.
fn texts(values: &[OsString]) -> Vec<String> {
    values.iter().map(|value| path_text(value)).collect()
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<NativeReuseAttempt>()?;
    module.add_function(wrap_pyfunction!(check_compile_reuse, module)?)
}
