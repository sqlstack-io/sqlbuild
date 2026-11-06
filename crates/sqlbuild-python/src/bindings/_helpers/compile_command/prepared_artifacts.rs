//! Prepare compile artifacts in the background and publish them only at the normal write phase.

use pyo3::exceptions::PyOSError;
use pyo3::prelude::{
    Bound, Py, PyAny, PyAnyMethods, PyModule, PyModuleMethods, PyRef, PyRefMut, PyResult, Python,
};
use pyo3::types::{PyCFunction, PyDict, PyFrozenSet, PyTuple};
use pyo3::{IntoPyObjectExt, pyclass, pymethods};

use crate::bindings::_helpers::boundary::python_calls::{
    KeywordArguments, call_with, imported, keywords,
};
use crate::bindings::_helpers::compile_command::reuse_recording::CompileArtifactWrites;
use crate::bindings::_helpers::compile_command::target_writer::{
    PlannedStaticSqlTests, plan_tests,
};
use crate::bindings::_helpers::compile_command::target_writer::{
    Prepared, planning_diagnostics, publish_staged_target, staging_changed, write_static_target,
};
use crate::bindings::_helpers::compile_driver::stage_work::{
    BACKGROUND_SQL_TEST_PLANS, count_stage,
};
use crate::bindings::_helpers::compile_outputs::artifacts::staged_artifact_files;
use crate::bindings::_helpers::records::fields::field as native_field;

const MIN_PREPARED_ARTIFACT_MODELS: usize = 128;
const MAX_PREPARED_ARTIFACT_MODELS: usize = 5000;
const STAGING_PREFIX: &str = ".sqlbuild-staging-";
const LOCK_SUFFIX: &str = ".lock";
const OWNER_MARKER: &str = ".owner";
const COMPILED_DIR: &str = "compiled";
/// The logger the retired Python class warned through, kept so log filters still apply.
const LOGGER_NAME: &str = "sqlbuild.cli.commands.classes.prepared_compile_artifacts";
const FILELOCK_MODULE: &str = "filelock";
const ARTIFACT_THREADS: &str = "sqlbuild-artifacts";
const TEST_THREADS: &str = "sqlbuild-tests";

/// One background preparation task: staged artifacts or planned SQL tests only.
#[pyclass(module = "sqlbuild._native", subclass)]
pub(crate) struct PreparedCompileArtifacts {
    enabled: bool,
    /// Where the compile notes the artifacts this task writes or keeps.
    artifact_writes: Option<Py<CompileArtifactWrites>>,
    staging_dir: Option<Py<PyAny>>,
    owner_lock: Option<Py<PyAny>>,
    created_target_dir: Option<Py<PyAny>>,
    executor: Option<Py<PyAny>>,
    future: Option<Py<PyAny>>,
    planned_tests: Option<Py<PyAny>>,
    planning_inputs: Option<(Py<PyAny>, Py<PyAny>, Py<PyAny>)>,
}

fn owner_lock_path<'py>(staging_dir: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let name: String = native_field(staging_dir, "name")?.extract()?;
    staging_dir.call_method1("with_name", (format!("{name}{LOCK_SUFFIX}"),))
}

fn owns_staging_directory(staging_dir: &Bound<'_, PyAny>) -> PyResult<bool> {
    let py: Python<'_> = staging_dir.py();
    let read: PyResult<Bound<'_, PyAny>> = staging_dir.div(OWNER_MARKER)?.call_method(
        "read_text",
        (),
        Some(&keywords(
            py,
            &[("encoding", "utf-8".into_bound_py_any(py)?)],
        )?),
    );
    match read {
        Ok(text) => text.eq(native_field(staging_dir, "name")?),
        Err(error) if error.is_instance_of::<PyOSError>(py) => Ok(false),
        Err(error) => Err(error),
    }
}

/// Release the lock after deleting its file, ignoring a file that cannot be deleted.
fn release_and_unlink(lock: &Bound<'_, PyAny>) -> PyResult<()> {
    let py: Python<'_> = lock.py();
    let unlinked: PyResult<Bound<'_, PyAny>> = imported(py, "pathlib", "Path")?
        .call1((native_field(lock, "lock_file")?,))?
        .call_method(
            "unlink",
            (),
            Some(&keywords(
                py,
                &[("missing_ok", true.into_bound_py_any(py)?)],
            )?),
        );
    if let Err(error) = unlinked
        && !error.is_instance_of::<PyOSError>(py)
    {
        return Err(error);
    }
    let _ = lock.call_method0("release")?;
    Ok(())
}

fn warn(
    py: Python<'_>,
    message: &str,
    arguments: (Bound<'_, PyAny>, Bound<'_, PyAny>),
) -> PyResult<()> {
    let _ = imported(py, "logging", "getLogger")?
        .call1((LOGGER_NAME,))?
        .call_method1("warning", (message, arguments.0, arguments.1))?;
    Ok(())
}

/// Remove one ownerless staging directory while holding its lock; `false` stops cleanup.
fn remove_abandoned_directory(directory: &Bound<'_, PyAny>) -> PyResult<bool> {
    let py: Python<'_> = directory.py();
    let lock: Bound<'_, PyAny> = imported(py, FILELOCK_MODULE, "FileLock")?.call(
        (owner_lock_path(directory)?,),
        Some(&keywords(py, &[("timeout", 0_i64.into_bound_py_any(py)?)])?),
    )?;
    if let Err(error) = lock.call_method0("acquire") {
        if error.is_instance(py, &imported(py, FILELOCK_MODULE, "Timeout")?) {
            return Ok(true);
        }
        if error.is_instance_of::<PyOSError>(py) {
            warn(
                py,
                "skipped abandoned staging cleanup in %s: %s",
                (
                    native_field(directory, "parent")?,
                    error.value(py).clone().into_any(),
                ),
            )?;
            return Ok(false);
        }
        return Err(error);
    }
    let removed: PyResult<()> = (|| {
        if directory.call_method0("is_dir")?.is_truthy()? {
            let _ = imported(py, "shutil", "rmtree")?.call1((directory,))?;
        }
        Ok(())
    })();
    if let Err(error) = removed {
        if !error.is_instance_of::<PyOSError>(py) {
            return Err(error);
        }
        let _ = lock.call_method0("release")?;
        warn(
            py,
            "stopped abandoned staging cleanup at %s: %s",
            (directory.clone(), error.value(py).clone().into_any()),
        )?;
        return Ok(false);
    }
    release_and_unlink(&lock)?;
    Ok(true)
}

/// `ThreadPoolExecutor(max_workers=1, thread_name_prefix=prefix)`.
fn single_thread_executor<'py>(py: Python<'py>, prefix: &str) -> PyResult<Bound<'py, PyAny>> {
    call_with(
        &imported(py, "concurrent.futures", "ThreadPoolExecutor")?,
        &[
            ("max_workers", 1_i64.into_bound_py_any(py)?),
            ("thread_name_prefix", prefix.into_bound_py_any(py)?),
        ],
    )
}

/// Submit `task` to run inside a copy of the current context, as the caller's work would.
fn submit_in_context<'py>(
    executor: &Bound<'py, PyAny>,
    task: Bound<'py, PyCFunction>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = executor.py();
    let context: Bound<'py, PyAny> = imported(py, "contextvars", "copy_context")?.call0()?;
    executor.call_method1("submit", (native_field(&context, "run")?, task))
}

fn is_os_error(py: Python<'_>, error: &pyo3::PyErr) -> bool {
    error.is_instance_of::<PyOSError>(py)
}

/// The background test plan's `result`, with its project, adapter and target directory.
type Planning<'py> = (Bound<'py, PyAny>, [Bound<'py, PyAny>; 3]);

impl PreparedCompileArtifacts {
    /// Where the compile notes this task's artifact writes.
    fn recorder(slf: &Bound<'_, Self>) -> Option<Py<CompileArtifactWrites>> {
        slf.borrow()
            .artifact_writes
            .as_ref()
            .map(|recorder| recorder.clone_ref(slf.py()))
    }

    /// Whether staging pays off: uncached, mid-sized projects whose memory stays within budget.
    fn stages_artifacts(slf: &Bound<'_, Self>, project: &Bound<'_, PyAny>) -> PyResult<bool> {
        let models: usize = native_field(project, "models")?.len()?;
        let minimum: usize = native_field(slf, "min_prepared_models")?.extract()?;
        let maximum: usize = native_field(slf, "max_prepared_models")?.extract()?;
        Ok(native_field(project, "compile_cache_dir")?.is_none()
            && minimum <= models
            && models <= maximum)
    }

    /// Stage artifacts beside `target_dir` so publication can move them instead of copying.
    fn start_staging<'py>(
        slf: &Bound<'py, Self>,
        project: &Bound<'py, PyAny>,
        adapter: &Bound<'py, PyAny>,
        target_dir: &Bound<'py, PyAny>,
    ) -> PyResult<bool> {
        let py: Python<'py> = slf.py();
        let hex: String = imported(py, "uuid", "uuid4")?
            .call0()?
            .getattr("hex")?
            .extract()?;
        let staging_dir: Bound<'py, PyAny> = target_dir.div(format!("{STAGING_PREFIX}{hex}"))?;
        let prepared: PyResult<()> = (|| {
            if !target_dir.call_method0("is_dir")?.is_truthy()? {
                let _ = target_dir.call_method(
                    "mkdir",
                    (),
                    Some(&keywords(py, &[("parents", true.into_bound_py_any(py)?)])?),
                )?;
                slf.borrow_mut().created_target_dir = Some(target_dir.clone().unbind());
            }
            let lock: Bound<'py, PyAny> = imported(py, FILELOCK_MODULE, "FileLock")?
                .call1((owner_lock_path(&staging_dir)?,))?;
            slf.borrow_mut().owner_lock = Some(lock.clone().unbind());
            let _ = lock.call_method0("acquire")?;
            let _ = slf.call_method1("create_staging_directory", (&staging_dir,))?;
            Ok(())
        })();
        if let Err(error) = prepared {
            if is_os_error(py, &error) {
                return Ok(false);
            }
            return Err(error);
        }
        let executor: Bound<'py, PyAny> = single_thread_executor(py, ARTIFACT_THREADS)?;
        let (project, adapter, staged) = (
            project.clone().unbind(),
            adapter.clone().unbind(),
            staging_dir.clone().unbind(),
        );
        let recorder: Option<Py<CompileArtifactWrites>> = Self::recorder(slf);
        let prepare = move |arguments: &Bound<'_, PyTuple>,
                            _: Option<&Bound<'_, PyDict>>|
              -> PyResult<Py<PyAny>> {
            let py: Python<'_> = arguments.py();
            let staging_dir: &Bound<'_, PyAny> = staged.bind(py);
            let written: Bound<'_, PyAny> = write_static_target(
                staging_dir,
                (adapter.bind(py), project.bind(py)),
                &py.None().into_bound(py),
                Prepared {
                    planned_tests: None,
                    recorder: recorder.as_ref(),
                },
            )?;
            let files: Vec<String> =
                staged_artifact_files(py, staging_dir.div(COMPILED_DIR)?.extract()?)?;
            Ok(
                PyTuple::new(py, [written, PyFrozenSet::new(py, &files)?.into_any()])?
                    .into_any()
                    .unbind(),
            )
        };
        let future: Bound<'py, PyAny> = submit_in_context(
            &executor,
            PyCFunction::new_closure(py, None, None, prepare)?,
        )?;
        let mut state: PyRefMut<'py, Self> = slf.borrow_mut();
        state.staging_dir = Some(staging_dir.unbind());
        state.executor = Some(executor.unbind());
        state.future = Some(future.unbind());
        Ok(true)
    }

    /// Plan SQL tests against the published target while the compile continues.
    fn start_test_planning<'py>(
        slf: &Bound<'py, Self>,
        project: &Bound<'py, PyAny>,
        adapter: &Bound<'py, PyAny>,
        target_dir: &Bound<'py, PyAny>,
    ) -> PyResult<()> {
        let py: Python<'py> = slf.py();
        let executor: Bound<'py, PyAny> = single_thread_executor(py, TEST_THREADS)?;
        let (planned_project, planned_adapter, planned_target) = (
            project.clone().unbind(),
            adapter.clone().unbind(),
            target_dir.clone().unbind(),
        );
        let recorder: Option<Py<CompileArtifactWrites>> = Self::recorder(slf);
        let plan = move |arguments: &Bound<'_, PyTuple>,
                         _: Option<&Bound<'_, PyDict>>|
              -> PyResult<Py<PyAny>> {
            let py: Python<'_> = arguments.py();
            count_stage(py, BACKGROUND_SQL_TEST_PLANS, 1)?;
            let planned: PlannedStaticSqlTests = plan_tests(
                planned_target.bind(py),
                (planned_adapter.bind(py), planned_project.bind(py)),
                recorder.as_ref(),
            )?;
            Ok(Py::new(py, planned)?.into_any())
        };
        let future: Bound<'py, PyAny> =
            submit_in_context(&executor, PyCFunction::new_closure(py, None, None, plan)?)?;
        let mut state: PyRefMut<'py, Self> = slf.borrow_mut();
        state.planning_inputs = Some((
            project.clone().unbind(),
            adapter.clone().unbind(),
            target_dir.clone().unbind(),
        ));
        state.executor = Some(executor.unbind());
        state.planned_tests = Some(future.unbind());
        Ok(())
    }

    /// The background test plan's `result` and its project, adapter and target, when planning.
    fn planning<'py>(slf: &Bound<'py, Self>) -> PyResult<Option<Planning<'py>>> {
        let py: Python<'py> = slf.py();
        let state: PyRef<'py, Self> = slf.borrow();
        let (Some(planned), Some((project, adapter, target_dir))) =
            (&state.planned_tests, &state.planning_inputs)
        else {
            return Ok(None);
        };
        Ok(Some((
            native_field(planned.bind(py), "result")?,
            [
                project.bind(py).clone(),
                adapter.bind(py).clone(),
                target_dir.bind(py).clone(),
            ],
        )))
    }

    /// The staged preparation's `(written, files)`, or `None` when staging failed with `OSError`.
    fn staged_result<'py>(future: &Bound<'py, PyAny>) -> PyResult<Option<Bound<'py, PyAny>>> {
        match future.call_method0("result") {
            Ok(result) => Ok(Some(result)),
            Err(error) if is_os_error(future.py(), &error) => Ok(None),
            Err(error) => Err(error),
        }
    }
}

#[pymethods]
impl PreparedCompileArtifacts {
    #[new]
    #[pyo3(signature = (*, enabled, artifact_writes=None))]
    fn new(enabled: bool, artifact_writes: Option<Py<CompileArtifactWrites>>) -> Self {
        Self {
            enabled,
            artifact_writes,
            staging_dir: None,
            owner_lock: None,
            created_target_dir: None,
            executor: None,
            future: None,
            planned_tests: None,
            planning_inputs: None,
        }
    }

    #[classattr]
    fn min_prepared_models() -> usize {
        MIN_PREPARED_ARTIFACT_MODELS
    }

    #[classattr]
    fn max_prepared_models() -> usize {
        MAX_PREPARED_ARTIFACT_MODELS
    }

    #[getter]
    pub(crate) fn enabled(&self) -> bool {
        self.enabled
    }

    fn __enter__(slf: Bound<'_, Self>) -> Bound<'_, Self> {
        slf
    }

    #[pyo3(signature = (*_exception))]
    fn __exit__(slf: &Bound<'_, Self>, _exception: &Bound<'_, PyTuple>) -> PyResult<()> {
        let py: Python<'_> = slf.py();
        let (executor, staging_dir, owner_lock, created) = {
            let state: PyRef<'_, Self> = slf.borrow();
            (
                state.executor.as_ref().map(|value| value.clone_ref(py)),
                state.staging_dir.as_ref().map(|value| value.clone_ref(py)),
                state.owner_lock.as_ref().map(|value| value.clone_ref(py)),
                state
                    .created_target_dir
                    .as_ref()
                    .map(|value| value.clone_ref(py)),
            )
        };
        let shutdown: PyResult<()> = match executor {
            Some(executor) => executor
                .bind(py)
                .call_method(
                    "shutdown",
                    (),
                    Some(&keywords(
                        py,
                        &[
                            ("wait", true.into_bound_py_any(py)?),
                            ("cancel_futures", true.into_bound_py_any(py)?),
                        ],
                    )?),
                )
                .map(|_| ()),
            None => Ok(()),
        };
        let cleanup: PyResult<()> = (|| {
            if let Some(staging_dir) = staging_dir {
                let _ = imported(py, "shutil", "rmtree")?.call(
                    (staging_dir,),
                    Some(&keywords(
                        py,
                        &[("ignore_errors", true.into_bound_py_any(py)?)],
                    )?),
                )?;
            }
            if let Some(lock) = owner_lock {
                release_and_unlink(lock.bind(py))?;
            }
            if let Some(directory) = created
                && let Err(error) = directory.bind(py).call_method0("rmdir")
                && !is_os_error(py, &error)
            {
                return Err(error);
            }
            Ok(())
        })();
        cleanup?;
        shutdown
    }

    /// Delete staging directories whose owner lock this compile can hold for the removal.
    #[pyo3(signature = (*, target_dir))]
    fn remove_abandoned_staging(&self, target_dir: &Bound<'_, PyAny>) -> PyResult<()> {
        if !self.enabled {
            return Ok(());
        }
        let py: Python<'_> = target_dir.py();
        let directories: Bound<'_, PyAny> = py
            .import("builtins")?
            .getattr("sorted")?
            .call1((target_dir.call_method1("glob", (format!("{STAGING_PREFIX}*"),))?,))?;
        for directory in directories.try_iter()? {
            let directory: Bound<'_, PyAny> = directory?;
            if native_field(&directory, "suffix")?.eq(LOCK_SUFFIX)? {
                continue;
            }
            if !remove_abandoned_directory(&directory)? {
                return Ok(());
            }
        }
        Ok(())
    }

    /// Create the staging directory and mark it with its own name as the owner.
    fn create_staging_directory(&self, staging_dir: &Bound<'_, PyAny>) -> PyResult<()> {
        let py: Python<'_> = staging_dir.py();
        let _ = staging_dir.call_method0("mkdir")?;
        let _ = staging_dir.div(OWNER_MARKER)?.call_method(
            "write_text",
            (native_field(staging_dir, "name")?,),
            Some(&keywords(
                py,
                &[("encoding", "utf-8".into_bound_py_any(py)?)],
            )?),
        )?;
        Ok(())
    }

    /// Stage artifacts beside `target_dir` when worthwhile, otherwise plan SQL tests ahead.
    #[pyo3(signature = (**options))]
    fn start(slf: &Bound<'_, Self>, options: Option<Bound<'_, PyDict>>) -> PyResult<()> {
        let arguments: KeywordArguments<'_> = KeywordArguments::read(
            slf.py(),
            "start",
            options,
            (
                &["project", "adapter", "target_dir", "stage_artifacts"],
                &[],
            ),
        )?;
        if !slf.borrow().enabled {
            return Ok(());
        }
        let (project, adapter, target_dir) = (
            &arguments.value("project")?,
            &arguments.value("adapter")?,
            &arguments.value("target_dir")?,
        );
        if arguments.flag("stage_artifacts")?
            && Self::stages_artifacts(slf, project)?
            && Self::start_staging(slf, project, adapter, target_dir)?
        {
            return Ok(());
        }
        if native_field(project, "sql_tests")?.is_truthy()? {
            Self::start_test_planning(slf, project, adapter, target_dir)?;
        }
        Ok(())
    }

    /// Publish what was prepared, or `None` when nothing was staged.
    #[pyo3(signature = (*, target_dir, manifest))]
    fn publish<'py>(
        slf: &Bound<'py, Self>,
        target_dir: &Bound<'py, PyAny>,
        manifest: &Bound<'py, PyAny>,
    ) -> PyResult<Option<Bound<'py, PyAny>>> {
        let py: Python<'py> = slf.py();
        if let Some((result, [project, adapter, _])) = Self::planning(slf)? {
            let recorder: Option<Py<CompileArtifactWrites>> = Self::recorder(slf);
            let prepared: Prepared<'_, 'py> = Prepared {
                planned_tests: Some(&result),
                recorder: recorder.as_ref(),
            };
            return write_static_target(target_dir, (&adapter, &project), manifest, prepared)
                .map(Some);
        }
        let (future, staging_dir) = {
            let state: PyRef<'py, Self> = slf.borrow();
            match (&state.future, &state.staging_dir) {
                (Some(future), Some(staging_dir)) => {
                    (future.bind(py).clone(), staging_dir.bind(py).clone())
                }
                _ => return Ok(None),
            }
        };
        let Some(result) = Self::staged_result(&future)? else {
            return Ok(None);
        };
        let (prepared, expected_files): (Bound<'py, PyAny>, Bound<'py, PyAny>) =
            result.extract()?;
        if !owns_staging_directory(&staging_dir)? {
            let name: String = native_field(&staging_dir, "name")?.extract()?;
            return Err(staging_changed(
                py,
                &format!(
                    "staging directory '{name}' was removed or replaced before publication; \
                     no artifact was published"
                ),
            )?);
        }
        let recorder: Option<Py<CompileArtifactWrites>> = Self::recorder(slf);
        publish_staged_target(
            (&prepared, &expected_files),
            target_dir,
            manifest,
            recorder.as_ref(),
        )
        .map(Some)
    }

    /// Background SQL test planning diagnostics, without publishing any artifact.
    fn planning_diagnostics<'py>(slf: &Bound<'py, Self>) -> PyResult<Option<Bound<'py, PyAny>>> {
        let py: Python<'py> = slf.py();
        if let Some((result, [project, adapter, target_dir])) = Self::planning(slf)? {
            let recorder: Option<Py<CompileArtifactWrites>> = Self::recorder(slf);
            let prepared: Prepared<'_, 'py> = Prepared {
                planned_tests: Some(&result),
                recorder: recorder.as_ref(),
            };
            return planning_diagnostics(&target_dir, (&adapter, &project), prepared)
                .map(|diagnostics| Some(diagnostics.into_any()));
        }
        let future: Option<Bound<'py, PyAny>> = slf
            .borrow()
            .future
            .as_ref()
            .map(|future| future.bind(py).clone());
        let Some(future) = future else {
            return Ok(None);
        };
        match Self::staged_result(&future)? {
            Some(result) => Ok(Some(native_field(&result.get_item(0)?, "diagnostics")?)),
            None => Ok(None),
        }
    }
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PreparedCompileArtifacts>()
}
