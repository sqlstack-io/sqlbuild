//! Helpers of the runs table recorder: its DDL, the run's identity facts and its row cells.

use crate::job_settings::main::named_jobs::named_jobs;
use crate::native_host::models::RunCommandScope;
use crate::native_host::models::RunIdentity;
use pyo3::exceptions::{PyKeyError, PyOSError, PyValueError};
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::{PyDict, PyDictMethods};
use sqlbuild_cli::person_login::main::signed_in_principal::signed_in_principal;
use sqlbuild_core::constants::RUNS_TABLE_NAME;
use sqlbuild_core::git_revisions::main::head_commit::head_commit;
use sqlbuild_python_base::boundary::main::imported::imported;
use sqlbuild_python_base::native_host::main::write_console::write_console;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_front::native_host::main::fixed_state_table_sql::fixed_state_table_sql;
use sqlbuild_python_front::native_host::main::integer_literal::integer_literal;
use sqlbuild_python_front::native_host::main::literal::literal;
use sqlbuild_state::runs::constants::{FIRST_ATTEMPT, NULLABLE_RUN_COLUMNS, RUN_COLUMN_TYPES};
use sqlbuild_state::runs::models::{RunNodeCounts, ServerRun};
use std::path::Path;
use std::process::{Command, Stdio};

/// The adapter method that marks the runs table as supported.
pub(crate) const CREATE_TABLE_METHOD: &str = "render_create_runs_table_sql";
/// The adapter method rendering the session's warehouse identity as SQL, or None without one.
pub(crate) const WAREHOUSE_USER_METHOD: &str = "render_current_warehouse_user";

/// Create-if-missing DDL for the runs table, with adapter-rendered column types.
#[pyfunction]
#[pyo3(signature = (*, database, schema, render_qualified_name, render_framework_type))]
pub(crate) fn build_runs_create_table_sql(
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

/// Whether the checkout at `directory` has changes; None when Git is not installed or cannot tell.
pub(crate) fn git_dirty(directory: &Path) -> std::io::Result<Option<bool>> {
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
pub(crate) fn texts(value: &Bound<'_, PyAny>) -> PyResult<Vec<String>> {
    value.try_iter()?.map(|item| item?.extract()).collect()
}

/// The keys of `--vars` in the order given; their values can hold secrets and are never read.
pub(crate) fn var_keys(cli_vars: &Bound<'_, PyAny>) -> PyResult<Vec<String>> {
    if cli_vars.is_none() {
        return Ok(Vec::new());
    }
    cli_vars
        .cast::<PyDict>()
        .map_err(|_| PyValueError::new_err("--vars is not a mapping"))?
        .iter()
        .map(|(key, _)| key.str()?.extract())
        .collect()
}

/// The text of a zero-argument Python call; None when the system cannot say (`OSError` or `KeyError`).
pub(crate) fn python_fact(
    py: Python<'_>,
    (module, name): (&str, &str),
) -> PyResult<Option<String>> {
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
pub(crate) fn principal(py: Python<'_>, project_dir: &Path) -> Option<String> {
    match signed_in_principal(project_dir) {
        Ok(principal) => principal,
        Err(error) => {
            let _ = write_console(
                py,
                "stderr",
                &format!("Warning: this run records no sqlbuild_user: {error}\n"),
            );
            None
        }
    }
}

/// The identity of one run from its request; `server` is set when a worker runs it for the server.
pub(crate) fn run_identity(
    request: &Bound<'_, PyAny>,
    scope: &RunCommandScope<'_>,
    (run_id, target_name): (String, Option<String>),
    server: Option<ServerRun>,
) -> PyResult<RunIdentity> {
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
    Ok(RunIdentity {
        run_id,
        attempt: server.as_ref().map_or(FIRST_ATTEMPT, |run| run.attempt),
        retry_of_run_id: server.as_ref().and_then(|run| run.retry_of_run_id.clone()),
        orchestration_run_id: server.map(|run| run.orchestration_run_id),
        command: scope.command,
        sqlbuild_version: scope.sqlbuild_version,
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

pub(crate) fn json_text<T: serde::Serialize>(value: &T) -> PyResult<String> {
    serde_json::to_string(value).map_err(|error| PyValueError::new_err(error.to_string()))
}

/// One row's cells in `RUN_COLUMN_TYPES` order; `finish` fills the finished row's status and counts.
pub(crate) fn row_cells(
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
        literal(py, Some(identity.sqlbuild_version))?,
        literal(py, identity.git_commit.as_deref())?,
        integer_literal(py, identity.git_dirty.map(i64::from))?,
        literal(py, Some(&json_text(&identity.var_keys)?))?,
        literal(py, finish.map(|(status, _)| status))?,
        literal(py, counts.as_deref())?,
        recorded_at.to_owned(),
    ])
}
