//! Public records of native host.

use crate::compiled_views::models::NativeCompiledModel;
use crate::native_host::_helpers::run_writes::{
    RowWrite, RunRow, ShareResult, warn_on_errors, write_shares,
};
use crate::native_host::_helpers::runs_state::CREATE_TABLE_METHOD;
use crate::planner_scope::models::ScopeData;
use pyo3::IntoPyObject;
use pyo3::PyErr;
use pyo3::exceptions::PyKeyboardInterrupt;
use pyo3::prelude::Bound;
use pyo3::prelude::Py;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyAnyMethods;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;
use sqlbuild_core::constants::RUNS_TABLE_NAME;
use sqlbuild_orchestration::declarations::models::TriggerDefinition;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::main::fields::mapping_get::mapping_get;
use sqlbuild_python_front::native_host::main::state_table_name::state_table_name;
use sqlbuild_python_front::native_host::models::Location;
use sqlbuild_state::runs::constants::{FINISHED_PHASE, STARTED_PHASE};
use sqlbuild_state::runs::main::run_status::run_status;
use sqlbuild_state::runs::models::{RunEnding, RunNodeCounts};
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

/// The runs table in one schema the run writes to.
#[derive(Debug)]
pub struct RunTable {
    pub location: Location,
    pub table: String,
}

/// A run's two rows in the runs table of each schema it writes to; a failed write only warns.
#[derive(Debug)]
pub struct RunRecorder<'py> {
    adapter: Bound<'py, PyAny>,
    connection_config: Bound<'py, PyAny>,
    tables: Vec<RunTable>,
    identity: RunIdentity,
}

impl<'py> RunRecorder<'py> {
    /// A recorder when the adapter keeps a runs table and the run writes a schema; never connects.
    pub fn open(
        (adapter, connection_config): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
        locations: Vec<Location>,
        identity: RunIdentity,
    ) -> PyResult<Option<Self>> {
        if !adapter.hasattr(CREATE_TABLE_METHOD)? {
            return Ok(None);
        }
        let render_qualified_name: Bound<'py, PyAny> =
            native_field(adapter, "render_qualified_name")?;
        let mut tables: Vec<RunTable> = Vec::with_capacity(locations.len());
        for location in locations {
            let keywords = location.keywords(adapter.py());
            if let Some(table) = state_table_name(
                &render_qualified_name,
                (&keywords[0].1, &keywords[1].1),
                RUNS_TABLE_NAME,
            )? {
                tables.push(RunTable { location, table });
            }
        }
        if tables.is_empty() {
            return Ok(None);
        }
        Ok(Some(Self {
            adapter: adapter.clone(),
            connection_config: connection_config.clone(),
            tables,
            identity,
        }))
    }

    /// Set each schema's table up once for the run and record that it started; only an interrupt escapes.
    pub fn started(&self) -> PyResult<()> {
        self.append_everywhere((STARTED_PHASE, None), RowWrite::SetUpFirst)
    }

    /// Record how the run ended and what its nodes did; only an interrupt escapes as an error.
    pub fn finished(&self, counts: &RunNodeCounts, ending: RunEnding) -> PyResult<()> {
        self.append_everywhere(
            (FINISHED_PHASE, Some((run_status(counts, ending), counts))),
            RowWrite::InsertFirst,
        )
    }

    /// Record a run that stopped on `error`: cancelled on an interrupt, else failed. Never errors.
    pub fn raised(&self, error: &PyErr) {
        let ending: RunEnding = if error.is_instance_of::<PyKeyboardInterrupt>(self.adapter.py()) {
            RunEnding::Interrupted
        } else {
            RunEnding::Raised
        };
        let _ = self.finished(&RunNodeCounts::default(), ending);
    }

    /// Append the row to every schema's table at once, then warn for each failed write.
    ///
    /// The first table is written on the calling thread, so an interrupt raised there stops the run
    /// as it does for a single schema; an interrupt anywhere else escapes once every write is done.
    fn append_everywhere(&self, row: RunRow<'_>, mode: RowWrite) -> PyResult<()> {
        let py: Python<'py> = self.adapter.py();
        let shares: Vec<Vec<usize>> = (0..self.tables.len()).map(|index| vec![index]).collect();
        let shared: Vec<ShareResult> = write_shares(
            (&self.adapter, &self.connection_config),
            (&self.tables, &self.identity),
            &shares,
            (row, mode),
        );
        warn_on_errors(py, (&self.tables, &self.identity), row.0, shared)?;
        py.check_signals()
    }
}
