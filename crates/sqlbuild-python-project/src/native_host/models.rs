//! Public records of native host.

use crate::compiled_views::models::NativeCompiledModel;
use crate::native_host::_helpers::runs_state::CREATE_TABLE_METHOD;
use crate::native_host::_helpers::runs_state::WAREHOUSE_USER_METHOD;
use crate::native_host::_helpers::runs_state::row_cells;
use crate::planner_scope::models::ScopeData;
use pyo3::IntoPyObject;
use pyo3::prelude::Bound;
use pyo3::prelude::Py;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyAnyMethods;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;
use sqlbuild_core::constants::RUNS_TABLE_NAME;
use sqlbuild_orchestration::declarations::models::TriggerDefinition;
use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::native_host::main::write_console::write_console;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::main::fields::mapping_get::mapping_get;
use sqlbuild_python_front::native_host::main::execute::execute;
use sqlbuild_python_front::native_host::main::literal::literal;
use sqlbuild_python_front::native_host::main::state_table_name::state_table_name;
use sqlbuild_python_front::native_host::models::Location;
use sqlbuild_state::runs::constants::{FINISHED_PHASE, RUN_COLUMN_TYPES, STARTED_PHASE};
use sqlbuild_state::runs::main::run_status::run_status;
use sqlbuild_state::runs::models::{RunEnding, RunNodeCounts};
use sqlbuild_state::state_tables::main::values_insert_sql::values_insert_sql;
use std::path::Path;
use std::sync::Arc;

/// Native compiled views stay typed; caller-authored protocol implementations remain public values.
#[derive(Debug)]
pub enum ModelHandle {
    View(Py<NativeCompiledModel>),
    Public(Py<PyAny>),
}

impl ModelHandle {
    pub fn read(value: &Bound<'_, PyAny>) -> Self {
        match value.cast::<NativeCompiledModel>() {
            Ok(value) => Self::View(value.clone().unbind()),
            Err(_) => Self::Public(value.clone().unbind()),
        }
    }

    pub fn bind<'py>(&self, py: Python<'py>) -> Bound<'py, PyAny> {
        match self {
            Self::View(value) => value.bind(py).clone().into_any(),
            Self::Public(value) => value.bind(py).clone(),
        }
    }

    pub fn clone_ref(&self, py: Python<'_>) -> Self {
        match self {
            Self::View(value) => Self::View(value.clone_ref(py)),
            Self::Public(value) => Self::Public(value.clone_ref(py)),
        }
    }
}

/// A public caller's mapping, or the compiler-owned models shared by native planning.
#[derive(Debug)]
pub enum ModelLookup {
    Public(Py<PyAny>),
    Native(Arc<ScopeData>),
}

impl ModelLookup {
    pub fn clone_ref(&self, py: Python<'_>) -> Self {
        match self {
            Self::Public(value) => Self::Public(value.clone_ref(py)),
            Self::Native(value) => Self::Native(value.clone()),
        }
    }

    pub fn get<'py>(&self, py: Python<'py>, name: &str) -> PyResult<Bound<'py, PyAny>> {
        match self {
            Self::Public(value) => mapping_get(value.bind(py), name),
            Self::Native(value) => Ok(value
                .model(py, name)
                .unwrap_or_else(|| py.None().into_bound(py))),
        }
    }

    pub fn project<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        match self {
            Self::Public(value) => Ok(value.bind(py).clone()),
            Self::Native(value) => value.field(py, "models_by_name"),
        }
    }
}

impl<'py> From<Bound<'py, PyAny>> for ModelLookup {
    fn from(value: Bound<'py, PyAny>) -> Self {
        Self::Public(value.unbind())
    }
}

impl<'py> IntoPyObject<'py> for &ModelLookup {
    type Target = PyAny;
    type Output = Bound<'py, PyAny>;
    type Error = pyo3::PyErr;

    fn into_pyobject(self, py: Python<'py>) -> PyResult<Self::Output> {
        self.project(py)
    }
}

/// A job's or group's schedule on a named calendar, as its trigger before the calendar's times fill it.
#[derive(Debug)]
pub struct CalendarRequest {
    pub calendar: String,
    pub timezone: String,
    pub trigger: TriggerDefinition,
}

/// A worker build's cost finalization, held until the run has published its outcome.
#[derive(Debug, Default)]
pub struct DeferredCost(std::sync::Mutex<Option<Py<PyAny>>>);

impl DeferredCost {
    const POISONED: &str = "deferred cost lock is poisoned";

    /// Keep `finalization`, replacing any earlier one.
    pub fn defer(&self, finalization: &Bound<'_, PyAny>) -> PyResult<()> {
        *self
            .0
            .lock()
            .map_err(|_| pyo3::exceptions::PyRuntimeError::new_err(Self::POISONED))? =
            Some(finalization.clone().unbind());
        Ok(())
    }

    /// The deferred finalization, once.
    pub fn take(&self) -> PyResult<Option<Py<PyAny>>> {
        Ok(self
            .0
            .lock()
            .map_err(|_| pyo3::exceptions::PyRuntimeError::new_err(Self::POISONED))?
            .take())
    }
}

/// What a run records about itself in both of its rows; `--vars` contributes only its keys.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunIdentity {
    pub run_id: String,
    pub attempt: i64,
    pub retry_of_run_id: Option<String>,
    pub orchestration_run_id: Option<String>,
    pub command: &'static str,
    /// The SQLBuild version that recorded the run.
    pub sqlbuild_version: &'static str,
    pub select: Vec<String>,
    pub exclude: Vec<String>,
    pub named_jobs: Vec<String>,
    pub target_name: Option<String>,
    pub sqlbuild_user: Option<String>,
    pub os_user: Option<String>,
    pub host: Option<String>,
    pub git_commit: Option<String>,
    pub git_dirty: Option<bool>,
    pub var_keys: Vec<String>,
}

/// One command's run: its name, project directory and whether the project records the host.
#[derive(Debug)]
pub struct RunCommandScope<'a> {
    pub command: &'static str,
    pub project_dir: &'a Path,
    pub record_host: bool,
    pub sqlbuild_version: &'static str,
}

/// A build's two rows in its target's runs table; a failed write warns and never fails the build.
#[derive(Debug)]
pub struct RunRecorder<'py> {
    adapter: Bound<'py, PyAny>,
    connection_config: Bound<'py, PyAny>,
    location: Location,
    table: String,
    identity: RunIdentity,
}

impl<'py> RunRecorder<'py> {
    /// A recorder when the adapter keeps a runs table and the target has a schema; never connects.
    pub fn open(
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
    pub fn started(&self) {
        self.warn_on_error(STARTED_PHASE, self.write(STARTED_PHASE, None, true));
    }

    /// Record how the run ended and what its nodes did.
    pub fn finished(&self, counts: &RunNodeCounts, ending: RunEnding) {
        let finish = (run_status(counts, ending), counts);
        let written: PyResult<()> = self
            .write(FINISHED_PHASE, Some(finish), false)
            .or_else(|_| self.write(FINISHED_PHASE, Some(finish), true));
        self.warn_on_error(FINISHED_PHASE, written);
    }

    fn warn_on_error(&self, phase: &str, written: PyResult<()>) {
        if let Err(error) = written {
            let _ = write_console(
                self.adapter.py(),
                "stderr",
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
