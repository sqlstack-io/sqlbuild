//! Public records of compile outputs.

use crate::compile_outputs::_helpers::artifacts::ArtifactGroup;
use crate::compile_outputs::_helpers::artifacts::DoneMarker;
use crate::compile_outputs::_helpers::artifacts::WRITER_THREAD;
use crate::compile_outputs::_helpers::artifacts::WriterDone;
use crate::compile_outputs::_helpers::artifacts::artifact_error;
use crate::compile_outputs::_helpers::artifacts::wait_for_writer;
use crate::compile_outputs::_helpers::artifacts::write_groups;
use crate::compile_outputs::_helpers::reuse::LaunchIdentityRow;
use crate::compile_outputs::_helpers::reuse::ReplayRow;
use crate::compile_outputs::_helpers::reuse::poisoned;
use crate::compile_outputs::_helpers::reuse::recorded_artifact;
use crate::compile_outputs::_helpers::reuse::settings_inputs;
use crate::compile_outputs::_helpers::reuse::texts;
use crate::compile_outputs::types::ArtifactRow;
use pyo3::FromPyObject;
use pyo3::IntoPyObject;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;
use pyo3::pyclass;
use pyo3::pymethods;
use sqlbuild_cache::artifacts::errors::ArtifactError;
use sqlbuild_cache::project_reuse::main::pending_digest_bytes::pending_digest_bytes;
use sqlbuild_cache::project_reuse::main::record_reuse::record_reuse;
use sqlbuild_cache::project_reuse::main::replay_stdout::replay_stdout;
use sqlbuild_cache::project_reuse::models::CompileRecord;
use sqlbuild_cache::project_reuse::models::HostIdentity;
use sqlbuild_cache::project_reuse::models::RecordOutcome;
use sqlbuild_cache::project_reuse::models::RecordResult;
use sqlbuild_cache::project_reuse::models::RecordedArtifact;
use sqlbuild_cache::project_reuse::models::ReuseAttempt;
use sqlbuild_cache::project_reuse::models::ReuseOutcome;
use sqlbuild_cache::project_reuse::models::ReuseRules;
use sqlbuild_cache::project_reuse::models::StoredOutput;
use sqlbuild_python_base::boundary::main::compiler_guard::compiler_guard;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::ffi::OsString;
use std::panic::resume_unwind;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::sync::mpsc::Sender;
use std::sync::mpsc::channel;
use std::thread::JoinHandle;
use std::time::Duration;

/// Artifact groups written one after another on their own thread, stopping at the first error.
#[derive(Debug)]
pub struct BackgroundArtifactWriter {
    pub(crate) groups: Sender<ArtifactGroup>,
    /// Set to skip every group not yet started; the group being written still completes.
    pub(crate) stop: Arc<AtomicBool>,
    /// Marked once the writer returns, so a wait can check for signals meanwhile.
    pub(crate) finished: Arc<WriterDone>,
    pub(crate) handle: JoinHandle<(Result<(), ArtifactError>, Duration)>,
}

impl BackgroundArtifactWriter {
    /// Start the writer thread, or `None` when no thread could start.
    pub fn start() -> Option<Self> {
        let (groups, received) = channel();
        let finished: Arc<WriterDone> = Arc::new(WriterDone::default());
        let marker: DoneMarker = DoneMarker(Arc::clone(&finished));
        let stop: Arc<AtomicBool> = Arc::new(AtomicBool::new(false));
        let stopping: Arc<AtomicBool> = Arc::clone(&stop);
        let spawned = std::thread::Builder::new()
            .name(WRITER_THREAD.to_owned())
            .spawn(move || {
                let _marker: DoneMarker = marker;
                write_groups(&received, &stopping)
            });
        match spawned {
            Ok(handle) => Some(Self {
                groups,
                stop,
                finished,
                handle,
            }),
            Err(_) => None,
        }
    }

    /// Queue one group behind the groups already sent; a writer that stopped drops it.
    pub fn send(&self, files: Vec<(PathBuf, Vec<u8>)>, check_existing: bool) {
        let _ = self.groups.send((files, check_existing));
    }

    /// Wait without the GIL for every sent group; a signal stops it after the current group.
    pub fn finish(self, py: Python<'_>) -> PyResult<Duration> {
        let Self {
            groups,
            stop,
            finished,
            handle,
        } = self;
        drop(groups);
        if let Err(interrupt) = wait_for_writer(py, &finished) {
            stop.store(true, Ordering::SeqCst);
            let _ = py.detach(|| handle.join());
            return Err(interrupt);
        }
        compiler_guard(|| {
            let joined = py.detach(|| handle.join());
            let (outcome, elapsed) = joined.unwrap_or_else(|payload| resume_unwind(payload));
            outcome.map_err(|error| artifact_error(py, error))?;
            Ok(elapsed)
        })
    }

    /// Stop after the group being written and wait for it without the GIL, ignoring the outcome.
    pub fn stop(self, py: Python<'_>) {
        let Self {
            groups,
            stop,
            finished,
            handle,
        } = self;
        stop.store(true, Ordering::SeqCst);
        drop(groups);
        drop(finished);
        let _ = py.detach(|| handle.join());
    }
}

/// Settings case sensitivity, names, prefixes, env files and secrets directories.
#[derive(Debug)]
pub struct SettingsRow {
    pub case_sensitive: bool,
    pub names: Vec<String>,
    pub prefixes: Vec<String>,
    pub env_files: Vec<String>,
    pub secrets_dirs: Vec<String>,
}

impl<'py> pyo3::FromPyObject<'py> for SettingsRow {
    fn extract_bound(value: &pyo3::Bound<'py, pyo3::PyAny>) -> pyo3::PyResult<Self> {
        let (case_sensitive, names, prefixes, env_files, secrets_dirs): (
            bool,
            Vec<String>,
            Vec<String>,
            Vec<String>,
            Vec<String>,
        ) = pyo3::types::PyAnyMethods::extract(value)?;
        Ok(Self {
            case_sensitive,
            names,
            prefixes,
            env_files,
            secrets_dirs,
        })
    }
}

/// The host constants the walk, environment and store checks use.
#[derive(Debug, FromPyObject, IntoPyObject)]
#[pyo3(from_item_all)]
pub struct ReuseRulesPy {
    pub excluded: Vec<String>,
    pub excluded_root: Vec<String>,
    pub presence_suffixes: Vec<String>,
    pub output_files: Vec<(u64, u64)>,
    pub tracked_prefixes: Vec<String>,
    pub untracked_names: Vec<String>,
    pub disabling_variables: Vec<(String, String)>,
    pub config_filenames: Vec<String>,
    pub racy_window_ns: i64,
    pub max_stored_entries: usize,
    pub max_entry_bytes: usize,
    pub missing_environment_value: String,
    pub missing_file_digest: String,
    pub missing_path_mtime_ns: i64,
    pub project_root_path_mtime_ns: i64,
    pub compiled_root: PathBuf,
    pub retired_directories: Vec<String>,
}

/// One reuse check: where the project and its store slot are, and the host's identity facts.
#[derive(Debug, FromPyObject)]
#[pyo3(from_item_all)]
pub struct ReuseCheckPy {
    pub project_dir: PathBuf,
    pub store_directory: PathBuf,
    pub selected_target: Option<OsString>,
    pub invocation: Vec<u8>,
    pub runtime: BTreeMap<String, OsString>,
    pub search_path: Vec<OsString>,
    pub bypass_requested: bool,
    pub json_output: bool,
    pub rules: ReuseRulesPy,
}

/// What a full compile leaves for recording; `artifacts` maps a path to `(digest, size, mtime)`.
#[derive(Debug, FromPyObject)]
#[pyo3(from_item_all)]
pub struct CompileRecordPy {
    pub stdout: Option<String>,
    pub stderr_lines: Vec<String>,
    pub exit_code: i64,
    pub compile_cache_enabled: bool,
    pub read_run_id: bool,
    pub settings_inputs: Option<Vec<SettingsRow>>,
    pub template_environment_names: Vec<String>,
    pub module_paths: Vec<OsString>,
    pub artifacts: HashMap<String, ArtifactRow>,
    pub artifacts_written: bool,
    pub dag_path: Option<String>,
    pub json_output: bool,
    pub rules: ReuseRulesPy,
}

/// One invocation's native reuse state, from the check to the record.
#[pyclass(module = "sqlbuild._native", frozen)]
#[derive(Debug)]
pub struct NativeReuseAttempt {
    pub(crate) attempt: Mutex<ReuseAttempt>,
}

impl NativeReuseAttempt {
    /// Run `body` on the native attempt.
    pub fn with_attempt<T>(&self, body: impl FnOnce(&ReuseAttempt) -> T) -> PyResult<T> {
        let attempt = self.attempt.lock().map_err(poisoned)?;
        Ok(body(&attempt))
    }
}

#[pymethods]
impl NativeReuseAttempt {
    /// `"hit"`, `"miss"` or `"bypass"`.
    pub(crate) fn outcome(&self) -> PyResult<&'static str> {
        Ok(match self.attempt.lock().map_err(poisoned)?.outcome {
            ReuseOutcome::Hit => "hit",
            ReuseOutcome::Miss => "miss",
            ReuseOutcome::Bypass => "bypass",
        })
    }

    /// The store file holding this invocation's slot.
    pub(crate) fn store_path(&self) -> PyResult<String> {
        let attempt = self.attempt.lock().map_err(poisoned)?;
        Ok(attempt.store_path.to_string_lossy().into_owned())
    }

    /// Whether a stored compile matched but could not be replayed.
    pub(crate) fn replay_failed(&self) -> PyResult<bool> {
        Ok(self.attempt.lock().map_err(poisoned)?.replay_failed)
    }

    /// How many project paths the check stamped.
    pub(crate) fn snapshot_paths(&self) -> PyResult<usize> {
        Ok(self.attempt.lock().map_err(poisoned)?.snapshot.len())
    }

    /// Bytes recording would hash for files whose stamp moved.
    pub(crate) fn pending_digest_bytes(&self) -> PyResult<u64> {
        let attempt = self.attempt.lock().map_err(poisoned)?;
        Ok(pending_digest_bytes(&attempt))
    }

    /// On a hit, the stored report with these timings plus its stderr lines and exit code.
    pub(crate) fn replay(&self, timings: Vec<(String, i64)>) -> PyResult<Option<ReplayRow>> {
        let attempt = self.attempt.lock().map_err(poisoned)?;
        let Some(output) = attempt.replay.as_ref() else {
            return Ok(None);
        };
        let output: &StoredOutput = output;
        Ok(replay_stdout(output, &timings)
            .map(|stdout| (stdout, output.stderr_lines.clone(), output.exit_code)))
    }

    /// The invocation digest, runtime and search path the check compared, for a launch record.
    pub(crate) fn launch_identity(&self) -> PyResult<LaunchIdentityRow> {
        let attempt = self.attempt.lock().map_err(poisoned)?;
        let identity: &HostIdentity = &attempt.identity;
        Ok((
            identity.invocation_digest.clone(),
            identity.runtime.clone(),
            identity.search_path.clone(),
        ))
    }

    /// How many project files the check read for content digests.
    pub(crate) fn digested_files(&self) -> PyResult<usize> {
        Ok(self.attempt.lock().map_err(poisoned)?.digested_files)
    }

    /// Store this compile or clear the slot; a failure clears the slot and raises `OSError`.
    pub fn record(
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
                image: sqlbuild_python_base::compile_context::main::delta::take_pending_image::take_pending_image(),
                vouched_target_stamps: None,
            };
            if let Some(merge) =
                sqlbuild_python_base::compile_context::main::delta::take_record_merge::take_record_merge()
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
