//! Public records of native host.

use crate::native_host::_helpers::consumption_io::append;
use crate::native_host::_helpers::consumption_io::ensure_table;
use crate::native_host::_helpers::consumption_io::read_executions;
use crate::native_host::_helpers::consumption_io::read_recent;
use crate::native_host::_helpers::consumption_state::text;
use crate::native_host::types::ConsumptionReads;
use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::Bound;
use pyo3::prelude::Py;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyAnyMethods;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_state::consumption::models::ConsumptionAppend;
use sqlbuild_state::consumption::models::ConsumptionRow;
use sqlbuild_state::consumption::models::RelationKey;
use std::sync::Mutex;

/// One destination schema of the consumption table.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Location {
    pub database: Option<String>,
    pub schema: String,
}

impl Location {
    pub fn of(relation: &RelationKey) -> Self {
        Self {
            database: relation.database.clone(),
            schema: relation.schema.clone(),
        }
    }

    pub fn keywords<'py>(&self, py: Python<'py>) -> Vec<(&'static str, Bound<'py, PyAny>)> {
        vec![
            (
                "database",
                self.database
                    .as_deref()
                    .map_or_else(|| py.None().into_bound(py), |database| text(py, database)),
            ),
            ("schema", text(py, &self.schema)),
        ]
    }
}

/// One build's consumption I/O, connected only per phase so loaders can lock file warehouses.
#[derive(Debug)]
pub struct ConsumptionConnection {
    pub(crate) adapter: Py<PyAny>,
    pub(crate) connection_config: Py<PyAny>,
    pub(crate) connection: Mutex<Option<Py<PyAny>>>,
}

impl ConsumptionConnection {
    pub fn new(adapter: &Bound<'_, PyAny>, connection_config: &Bound<'_, PyAny>) -> Self {
        Self {
            adapter: adapter.clone().unbind(),
            connection_config: connection_config.clone().unbind(),
            connection: Mutex::new(None),
        }
    }

    pub fn connected<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let mut open = self
            .connection
            .lock()
            .map_err(|_| PyRuntimeError::new_err("consumption connection lock poisoned"))?;
        if let Some(connection) = open.as_ref() {
            return Ok(connection.bind(py).clone());
        }
        let connection: Bound<'py, PyAny> = native_field(self.adapter.bind(py), "connect")?
            .call1((self.connection_config.bind(py),))?;
        *open = Some(connection.clone().unbind());
        Ok(connection)
    }

    pub fn ensure_table(&self, py: Python<'_>, location: &Location) -> PyResult<()> {
        ensure_table(self.adapter.bind(py), &self.connected(py)?, location)
    }

    pub fn read_recent(
        &self,
        py: Python<'_>,
        location: &Location,
    ) -> PyResult<Vec<ConsumptionRow>> {
        read_recent(self.adapter.bind(py), &self.connected(py)?, location)
    }

    pub fn read_executions(
        &self,
        py: Python<'_>,
        location: &Location,
        execution_ids: &[&str],
    ) -> PyResult<Vec<ConsumptionRow>> {
        read_executions(
            self.adapter.bind(py),
            &self.connected(py)?,
            location,
            execution_ids,
        )
    }

    pub fn append(
        &self,
        py: Python<'_>,
        location: &Location,
        rows: &[ConsumptionAppend],
    ) -> PyResult<()> {
        append(self.adapter.bind(py), &self.connected(py)?, location, rows)
    }

    /// Close the connection if one is open; the next operation connects again.
    pub fn close(&self, py: Python<'_>) -> PyResult<()> {
        let open: Option<Py<PyAny>> = self
            .connection
            .lock()
            .map_err(|_| PyRuntimeError::new_err("consumption connection lock poisoned"))?
            .take();
        if let Some(connection) = open {
            native_field(self.adapter.bind(py), "close")?.call1((connection.bind(py),))?;
        }
        Ok(())
    }
}

impl ConsumptionReads for ConsumptionConnection {
    fn read_recent(&self, py: Python<'_>, location: &Location) -> PyResult<Vec<ConsumptionRow>> {
        ConsumptionConnection::read_recent(self, py, location)
    }

    fn read_executions(
        &self,
        py: Python<'_>,
        location: &Location,
        execution_ids: &[&str],
    ) -> PyResult<Vec<ConsumptionRow>> {
        ConsumptionConnection::read_executions(self, py, location, execution_ids)
    }
}

/// The build's own node-completion callback, which the recorder forwards every result to.
#[derive(Debug)]
pub struct NodeCompleteHook(Py<PyAny>);

impl NodeCompleteHook {
    pub fn new(callback: &Bound<'_, PyAny>) -> Self {
        Self(callback.clone().unbind())
    }

    pub fn forward(&self, result: &Bound<'_, PyAny>) -> PyResult<()> {
        self.0.bind(result.py()).call1((result,))?;
        Ok(())
    }
}

/// One observation to append; the warehouse clock fills `observed_at`.
#[derive(Debug)]
pub struct ObservationAppend {
    pub kind: &'static str,
    pub node: String,
    pub cursor_type: Option<String>,
    pub value: String,
    pub run_id: String,
}
