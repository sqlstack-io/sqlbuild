//! DDL and best-effort appends of the runs table, which shows builds the control plane did not start.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use pyo3::PyErr;
use pyo3::exceptions::{PyKeyError, PyKeyboardInterrupt, PyOSError, PyValueError};
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyModule, PyModuleMethods, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods};
use pyo3::{pyfunction, wrap_pyfunction};
use sqlbuild_cli::person_login::main::signed_in_principal::signed_in_principal;
use sqlbuild_core::constants::RUNS_TABLE_NAME;
use sqlbuild_core::git_revisions::main::head_commit::head_commit;
use sqlbuild_host::models::BuildExecutionResult;
use sqlbuild_state::runs::constants::{
    FINISHED_PHASE, FIRST_ATTEMPT, NULLABLE_RUN_COLUMNS, RUN_COLUMN_TYPES, STARTED_PHASE,
};
use sqlbuild_state::runs::main::run_status::run_status;
use sqlbuild_state::runs::models::{RunEnding, RunNodeCounts, ServerRun};
use sqlbuild_state::state_tables::main::values_insert_sql::values_insert_sql;

use crate::bindings::_helpers::boundary::python_calls::{call_with, imported};
use crate::bindings::_helpers::compiled_project::job_catalog::named_jobs;
use crate::bindings::_helpers::native_host::consumption_io::execute;
use crate::bindings::_helpers::native_host::consumption_state::{
    Location, fixed_state_table_sql, integer_literal, literal, state_table_name,
};
use crate::bindings::_helpers::recall::console::write_stderr;
use crate::bindings::_helpers::records::fields::field as native_field;

/// The adapter method that marks the runs table as supported.
const CREATE_TABLE_METHOD: &str = "render_create_runs_table_sql";
/// The adapter method rendering the session's warehouse identity as SQL, or None without one.
const WAREHOUSE_USER_METHOD: &str = "render_current_warehouse_user";

/// Create-if-missing DDL for the runs table, with adapter-rendered column types.
#[pyfunction]
#[pyo3(signature = (*, database, schema, render_qualified_name, render_framework_type))]
fn build_runs_create_table_sql(
    database: &Bound<'_, PyAny>,
    schema: &Bound<'_, PyAny>,
    render_qualified_name: &Bound<'_, PyAny>,
    render_framework_type: &Bound<'_, PyAny>,
) -> PyResult<String> {
    fixed_state_table_sql(
        (database, schema),
        (render_qualified_name, render_framework_type),
        (RUNS_TABLE_NAME, &RUN_COLUMN_TYPES, &NULLABLE_RUN_COLUMNS),
    )
}

/// What a run records about itself in both of its rows; `--vars` contributes only its keys.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RunIdentity {
    pub(crate) run_id: String,
    pub(crate) attempt: i64,
    pub(crate) retry_of_run_id: Option<String>,
    pub(crate) orchestration_run_id: Option<String>,
    pub(crate) command: &'static str,
    pub(crate) select: Vec<String>,
    pub(crate) exclude: Vec<String>,
    pub(crate) named_jobs: Vec<String>,
    pub(crate) target_name: Option<String>,
    pub(crate) sqlbuild_user: Option<String>,
    pub(crate) os_user: Option<String>,
    pub(crate) host: Option<String>,
    pub(crate) git_commit: Option<String>,
    pub(crate) git_dirty: Option<bool>,
    pub(crate) var_keys: Vec<String>,
}

/// One command's run: its name, project directory and whether the project records the host.
pub(crate) struct RunCommandScope<'a> {
    pub(crate) command: &'static str,
    pub(crate) project_dir: &'a Path,
    pub(crate) record_host: bool,
}

/// Whether the checkout at `directory` has changes; None when Git is not installed or cannot tell.
fn git_dirty(directory: &Path) -> std::io::Result<Option<bool>> {
    let output = match Command::new("git")
        .current_dir(directory)
        .args(["status", "--porcelain", "--untracked-files=normal"])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
    {
        Ok(output) => output,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    Ok(output
        .status
        .success()
        .then(|| !String::from_utf8_lossy(&output.stdout).trim().is_empty()))
}

/// The strings of a Python iterable of selectors.
fn texts(value: &Bound<'_, PyAny>) -> PyResult<Vec<String>> {
    value.try_iter()?.map(|item| item?.extract()).collect()
}

/// The keys of `--vars` in the order given; their values can hold secrets and are never read.
fn var_keys(cli_vars: &Bound<'_, PyAny>) -> PyResult<Vec<String>> {
    if cli_vars.is_none() {
        return Ok(Vec::new());
    }
    cli_vars
        .downcast::<PyDict>()
        .map_err(|_| PyValueError::new_err("--vars is not a mapping"))?
        .iter()
        .map(|(key, _)| key.str()?.extract())
        .collect()
}

/// The text of a zero-argument Python call; None when the system cannot say (`OSError` or `KeyError`).
fn python_fact(py: Python<'_>, (module, name): (&str, &str)) -> PyResult<Option<String>> {
    match imported(py, module, name)?.call0() {
        Ok(value) => Ok(Some(value.extract::<String>()?).filter(|value| !value.is_empty())),
        Err(error)
            if error.is_instance_of::<PyOSError>(py) || error.is_instance_of::<PyKeyError>(py) =>
        {
            Ok(None)
        }
        Err(error) => Err(error),
    }
}

/// The `sqb login` principal; a credential store that cannot be read only warns.
fn principal(py: Python<'_>, project_dir: &Path) -> Option<String> {
    match signed_in_principal(project_dir) {
        Ok(principal) => principal,
        Err(error) => {
            let _ = write_stderr(
                py,
                &format!("Warning: this run records no sqlbuild_user: {error}\n"),
            );
            None
        }
    }
}

impl RunIdentity {
    /// The identity of one run from its request; `server` is set when a worker runs it for the server.
    pub(crate) fn collect(
        request: &Bound<'_, PyAny>,
        scope: &RunCommandScope<'_>,
        (run_id, target_name): (String, Option<String>),
        server: Option<ServerRun>,
    ) -> PyResult<Self> {
        let py: Python<'_> = request.py();
        let project_dir: &Path = scope.project_dir;
        let select: Vec<String> = texts(&native_field(request, "select")?)?;
        let (git_commit, git_dirty) = py.detach(|| -> std::io::Result<_> {
            let commit: Option<String> = head_commit(project_dir);
            let dirty: Option<bool> = match commit {
                Some(_) => git_dirty(project_dir)?,
                None => None,
            };
            Ok((commit, dirty))
        })?;
        let sqlbuild_user: Option<String> = match &server {
            Some(run) => Some(run.sqlbuild_user.clone()),
            None => principal(py, project_dir),
        };
        Ok(Self {
            run_id,
            attempt: server.as_ref().map_or(FIRST_ATTEMPT, |run| run.attempt),
            retry_of_run_id: server.as_ref().and_then(|run| run.retry_of_run_id.clone()),
            orchestration_run_id: server.map(|run| run.orchestration_run_id),
            command: scope.command,
            named_jobs: named_jobs(py, &select)?,
            select,
            exclude: texts(&native_field(request, "exclude")?)?,
            target_name,
            sqlbuild_user,
            os_user: python_fact(py, ("getpass", "getuser"))?,
            host: if scope.record_host {
                python_fact(py, ("socket", "gethostname"))?
            } else {
                None
            },
            git_commit,
            git_dirty,
            var_keys: var_keys(&native_field(request, "cli_vars")?)?,
        })
    }
}

fn json_text<T: serde::Serialize>(value: &T) -> PyResult<String> {
    serde_json::to_string(value).map_err(|error| PyValueError::new_err(error.to_string()))
}

/// One row's cells in `RUN_COLUMN_TYPES` order; `finish` fills the finished row's status and counts.
fn row_cells(
    py: Python<'_>,
    identity: &RunIdentity,
    (phase, finish): (&str, Option<(&str, &RunNodeCounts)>),
    (warehouse_user, recorded_at): (&str, &str),
) -> PyResult<Vec<String>> {
    let counts: Option<String> = finish.map(|(_, counts)| json_text(counts)).transpose()?;
    Ok(vec![
        literal(py, Some(&identity.run_id))?,
        literal(py, Some(phase))?,
        integer_literal(py, Some(identity.attempt))?,
        literal(py, identity.retry_of_run_id.as_deref())?,
        literal(py, identity.orchestration_run_id.as_deref())?,
        literal(py, Some(identity.command))?,
        literal(py, Some(&json_text(&identity.select)?))?,
        literal(py, Some(&json_text(&identity.exclude)?))?,
        literal(py, Some(&json_text(&identity.named_jobs)?))?,
        literal(py, identity.target_name.as_deref())?,
        warehouse_user.to_owned(),
        literal(py, identity.sqlbuild_user.as_deref())?,
        literal(py, identity.os_user.as_deref())?,
        literal(py, identity.host.as_deref())?,
        literal(py, Some(env!("CARGO_PKG_VERSION")))?,
        literal(py, identity.git_commit.as_deref())?,
        integer_literal(py, identity.git_dirty.map(i64::from))?,
        literal(py, Some(&json_text(&identity.var_keys)?))?,
        literal(py, finish.map(|(status, _)| status))?,
        literal(py, counts.as_deref())?,
        recorded_at.to_owned(),
    ])
}

/// A build's two rows in its target's runs table; a failed write warns and never fails the build.
pub(crate) struct RunRecorder<'py> {
    adapter: Bound<'py, PyAny>,
    connection_config: Bound<'py, PyAny>,
    location: Location,
    table: String,
    identity: RunIdentity,
}

impl<'py> RunRecorder<'py> {
    /// A recorder when the adapter keeps a runs table and the target has a schema; never connects.
    pub(crate) fn open(
        (adapter, connection_config): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
        (database, schema): (Option<String>, Option<String>),
        identity: RunIdentity,
    ) -> PyResult<Option<Self>> {
        let Some(schema) = schema else {
            return Ok(None);
        };
        if !adapter.hasattr(CREATE_TABLE_METHOD)? {
            return Ok(None);
        }
        let location = Location { database, schema };
        let keywords = location.keywords(adapter.py());
        let Some(table) = state_table_name(
            &native_field(adapter, "render_qualified_name")?,
            (&keywords[0].1, &keywords[1].1),
            RUNS_TABLE_NAME,
        )?
        else {
            return Ok(None);
        };
        Ok(Some(Self {
            adapter: adapter.clone(),
            connection_config: connection_config.clone(),
            location,
            table,
            identity,
        }))
    }

    /// Record that the run started, creating the schema and table when missing.
    pub(crate) fn started(&self) {
        self.warn_on_error(STARTED_PHASE, self.write(STARTED_PHASE, None, true));
    }

    /// Record how the run ended and what its nodes did.
    pub(crate) fn finished(&self, counts: &RunNodeCounts, ending: RunEnding) {
        let finish = (run_status(counts, ending), counts);
        let written: PyResult<()> = self
            .write(FINISHED_PHASE, Some(finish), false)
            .or_else(|_| self.write(FINISHED_PHASE, Some(finish), true));
        self.warn_on_error(FINISHED_PHASE, written);
    }

    fn warn_on_error(&self, phase: &str, written: PyResult<()>) {
        if let Err(error) = written {
            let _ = write_stderr(
                self.adapter.py(),
                &format!(
                    "Warning: could not record that run {} {phase} in {RUNS_TABLE_NAME}: {error}\n",
                    self.identity.run_id
                ),
            );
        }
    }

    /// Connect for this row only, so loaders can still lock file warehouses during the build.
    fn write(
        &self,
        phase: &str,
        finish: Option<(&str, &RunNodeCounts)>,
        ensure_table: bool,
    ) -> PyResult<()> {
        let connection: Bound<'py, PyAny> =
            native_field(&self.adapter, "connect")?.call1((&self.connection_config,))?;
        let written: PyResult<()> = self.append(&connection, (phase, finish), ensure_table);
        let closed: PyResult<Bound<'py, PyAny>> =
            native_field(&self.adapter, "close")?.call1((&connection,));
        written?;
        closed.map(|_| ())
    }

    fn append(
        &self,
        connection: &Bound<'py, PyAny>,
        row: (&str, Option<(&str, &RunNodeCounts)>),
        ensure_table: bool,
    ) -> PyResult<()> {
        let py: Python<'py> = self.adapter.py();
        if ensure_table {
            let keywords = self.location.keywords(py);
            for statement in call_with(
                &native_field(&self.adapter, "render_create_schema")?,
                &keywords,
            )?
            .try_iter()?
            {
                execute(&self.adapter, connection, &statement?.extract::<String>()?)?;
            }
            let create: String = call_with(
                &native_field(&self.adapter, CREATE_TABLE_METHOD)?,
                &keywords,
            )?
            .extract()?;
            if !create.is_empty() {
                execute(&self.adapter, connection, &create)?;
            }
        }
        let recorded_at: String = call_with(
            &native_field(&self.adapter, "render_current_utc_timestamp")?,
            &[],
        )?
        .extract()?;
        let warehouse_user: String =
            match call_with(&native_field(&self.adapter, WAREHOUSE_USER_METHOD)?, &[])?
                .extract::<Option<String>>()?
            {
                Some(expression) => expression,
                None => literal(py, None)?,
            };
        let columns: Vec<&str> = RUN_COLUMN_TYPES.iter().map(|(column, _)| *column).collect();
        let cells: Vec<String> =
            row_cells(py, &self.identity, row, (&warehouse_user, &recorded_at))?;
        execute(
            &self.adapter,
            connection,
            &values_insert_sql(&self.table, &columns, &[cells]),
        )
        .map(|_| ())
    }
}

/// A completed build's node counts, as its footer counts them; every planned node reports a status.
pub(crate) fn build_node_counts<V>(result: &BuildExecutionResult<V>) -> RunNodeCounts {
    RunNodeCounts {
        succeeded: result.success_count,
        warned: result.warning_count,
        failed: result.failure_count,
        skipped: result.skipped_count,
        not_started: 0,
    }
}

/// How a run that raised `error` ended: an interrupt, including a worker's cancel, or an error.
pub(crate) fn raised_ending(py: Python<'_>, error: &PyErr) -> RunEnding {
    if error.is_instance_of::<PyKeyboardInterrupt>(py) {
        RunEnding::Interrupted
    } else {
        RunEnding::Raised
    }
}

/// One run's recorder, the same for the CLI and a worker (which passes `server`); failures only warn.
pub(crate) fn run_recorder<'py>(
    command: &'static str,
    (request, invocation, project): (&Bound<'py, PyAny>, &Bound<'py, PyAny>, &Bound<'py, PyAny>),
    server: Option<ServerRun>,
) -> Option<RunRecorder<'py>> {
    let opened = || -> PyResult<Option<RunRecorder<'py>>> {
        let project_dir: PathBuf = native_field(invocation, "effective_project_dir")?.extract()?;
        let settings: Bound<'py, PyAny> = native_field(
            &native_field(
                &native_field(invocation, "discovered_inputs")?,
                "project_config",
            )?,
            "settings",
        )?;
        let scope = RunCommandScope {
            command,
            project_dir: &project_dir,
            record_host: native_field(&settings, "record_run_host")?.is_truthy()?,
        };
        let identity: RunIdentity = RunIdentity::collect(
            request,
            &scope,
            (
                native_field(project, "run_id")?.str()?.extract()?,
                native_field(project, "effective_target_name")?.extract()?,
            ),
            server,
        )?;
        RunRecorder::open(
            (
                &native_field(invocation, "adapter")?,
                &native_field(invocation, "connection_config")?,
            ),
            (
                native_field(project, "effective_target_database")?.extract()?,
                native_field(project, "effective_target_schema")?.extract()?,
            ),
            identity,
        )
    };
    opened().unwrap_or_else(|error| {
        let _ = write_stderr(
            request.py(),
            &format!("Warning: this run is not recorded in {RUNS_TABLE_NAME}: {error}\n"),
        );
        None
    })
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(build_runs_create_table_sql, module)?)
}
