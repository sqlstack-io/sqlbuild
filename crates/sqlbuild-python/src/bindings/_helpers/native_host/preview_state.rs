//! A plan preview's connection: it carries the synced warehouse state and refuses every warehouse use.

use std::cell::Cell;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex, PoisonError};

use chrono::SecondsFormat;
use pyo3::create_exception;
use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, pyclass, pymethods};
use sqlbuild_state::consumption::models::ConsumptionRow;
use sqlbuild_state::warehouse_state::models::{InvalidObservation, StateLocation, WarehouseState};

create_exception!(
    sqlbuild._native,
    WarehouseAccessRefusedError,
    PyRuntimeError,
    "Raised when a plan preview reaches past the synced warehouse state for the warehouse."
);

/// Why a plan preview leaves a source's freshness unknown; its plan lists freshness as unknown.
pub(crate) const PREVIEW_MESSAGE: &str = "a plan preview does not observe source freshness";
/// Items a preview reports as known only once the run plans live.
const PREVIEW_UNKNOWNS: [&str; 4] = [
    "source freshness",
    "retention",
    "column rename support",
    "sources outside mirrored schemas",
];

thread_local! {
    static PREVIEWING: Cell<bool> = const { Cell::new(false) };
}

/// While held, the plan on this thread is a preview; planner code without a connection asks `active`.
pub(crate) struct PreviewScope {
    previous: bool,
}

impl PreviewScope {
    pub(crate) fn enter() -> Self {
        Self {
            previous: PREVIEWING.replace(true),
        }
    }

    /// Whether the plan on this thread is a preview from the synced state.
    pub(crate) fn active() -> bool {
        PREVIEWING.get()
    }
}

impl Drop for PreviewScope {
    fn drop(&mut self) {
        PREVIEWING.set(self.previous);
    }
}

/// The error a preview raises for any read that would reach the warehouse.
pub(crate) fn refused(access: &str) -> pyo3::PyErr {
    WarehouseAccessRefusedError::new_err(format!(
        "plan previews read only the synced warehouse state; refused warehouse access: {access}"
    ))
}

/// The connection a preview plans through: the synced state, and what the plan found unknown.
#[pyclass(
    frozen,
    module = "sqlbuild.adapter.contract",
    name = "PreviewConnection"
)]
pub(crate) struct PreviewConnection {
    state: Arc<WarehouseState>,
    unknowns: Mutex<BTreeSet<String>>,
    windows: Mutex<BTreeMap<String, String>>,
}

/// The preview connection `connection` is, or `None` for a live warehouse connection.
pub(crate) fn preview_of<'py>(
    connection: &Bound<'py, PyAny>,
) -> Option<Bound<'py, PreviewConnection>> {
    connection.downcast::<PreviewConnection>().ok().cloned()
}

impl PreviewConnection {
    /// A preview over `state`, which several plans may share.
    pub(crate) fn new(state: Arc<WarehouseState>) -> Self {
        Self {
            state,
            unknowns: Mutex::new(BTreeSet::new()),
            windows: Mutex::new(BTreeMap::new()),
        }
    }

    /// The synced state the preview plans from.
    pub(crate) fn state(&self) -> &WarehouseState {
        &self.state
    }

    /// When the state was synced, as ISO 8601 text; `None` before every required table synced.
    pub(crate) fn synced_at(&self) -> Option<String> {
        self.state
            .synced_at()
            .map(|at| at.to_rfc3339_opts(SecondsFormat::AutoSi, true))
    }

    /// What the preview could not know, in a stable order.
    pub(crate) fn unknowns(&self) -> Vec<String> {
        let mut unknowns: Vec<String> = PREVIEW_UNKNOWNS
            .iter()
            .map(|item| (*item).to_owned())
            .collect();
        unknowns.extend(
            self.unknowns
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .iter()
                .cloned(),
        );
        unknowns
    }

    /// Note one item the preview reports as unknown until the run plans live.
    pub(crate) fn mark_unknown(&self, item: String) {
        self.unknowns
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(item);
    }

    /// Note that `model` reads from `start` up to the latest data when the run starts.
    pub(crate) fn mark_window(&self, model: &str, start: &str) {
        self.windows
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(model.to_owned(), start.to_owned());
    }

    /// Each model's window start, read up to the latest data when the run starts.
    pub(crate) fn windows(&self) -> BTreeMap<String, String> {
        self.windows
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// The model's cursor `MAX` text, only when observed by its newest successful build.
    pub(crate) fn target_max(&self, model: &str) -> Option<&str> {
        self.state.target_max(model)
    }

    /// Every synced consumption row of the table in `(database, schema)`.
    pub(crate) fn consumption(
        &self,
        (database, schema): (Option<&str>, &str),
    ) -> Vec<ConsumptionRow> {
        self.state.consumption_rows(&StateLocation {
            database: database.map(str::to_owned),
            schema: schema.to_owned(),
        })
    }

    /// The source's columns as last described, only when it described exactly `sql`.
    pub(crate) fn described_columns(
        &self,
        source: &str,
        sql: &str,
    ) -> PyResult<Option<Vec<String>>> {
        self.state.described_columns(source, sql).map_err(
            |InvalidObservation { source, reason }| {
                PyRuntimeError::new_err(format!(
                    "invalid source_columns observation for {source}: {reason}"
                ))
            },
        )
    }
}

#[pymethods]
impl PreviewConnection {
    /// Any use as a warehouse connection is refused: a preview reads only the synced state.
    fn __getattr__(&self, name: &str) -> PyResult<()> {
        Err(refused(&format!("connection.{name}")))
    }
}
