//! A plan preview's warehouse reads: answered from the synced warehouse state, never the warehouse.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex, PoisonError};

use chrono::SecondsFormat;
use pyo3::IntoPyObject;
use pyo3::create_exception;
use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods, PyString, PyTuple};
use sqlbuild_host::entries::models::AdapterHost;
use sqlbuild_state::consumption::constants::RECENT_EXECUTIONS;
use sqlbuild_state::consumption::main::recent_executions::recent_executions;
use sqlbuild_state::consumption::models::ConsumptionRow;
use sqlbuild_state::warehouse_state::models::{
    InvalidObservation, RelationFilter, StateFingerprint, StateLocation, StateRelation,
    WarehouseState,
};

use crate::bindings::_helpers::boundary::python_calls::call_with;
use crate::bindings::_helpers::fingerprints::latest_rows::selected_set;
use crate::bindings::_helpers::native_host::consumption_state::Location;
use crate::bindings::_helpers::native_host::host;
use crate::bindings::_helpers::planner_warehouse::reads::{
    CursorProgress, EligibleCursorQuery, FingerprintRead, Read, WarehouseReads, unknown,
};
use crate::bindings::_helpers::records::construct::{new_record_with, record_class};
use crate::bindings::_helpers::records::fields::field as native_field;
use crate::bindings::_helpers::records::ids::RecordId;
use crate::bindings::_helpers::state_rows::models::EventTableLayout;

create_exception!(
    sqlbuild._native,
    WarehouseAccessRefusedError,
    PyRuntimeError,
    "Raised when a plan preview reaches past the synced warehouse state for the warehouse."
);

/// Why a plan preview leaves a source's freshness unknown; its plan lists freshness as unknown.
pub(crate) const PREVIEW_MESSAGE: &str = "a plan preview does not observe source freshness";
/// Items a preview always reports as known only once the run plans live.
const PREVIEW_UNKNOWNS: [&str; 5] = [
    "source freshness",
    "retention",
    "column rename support",
    "sources outside mirrored schemas",
    "recorded migrations",
];
/// What a preview's cursor reads leave to the run.
const CURSOR_INPUTS: &str = "cursor inputs";

/// The reads of one plan preview over a synced state, and what the plan found unknown.
pub(crate) struct SyncedReads {
    state: Arc<WarehouseState>,
    unknowns: Mutex<BTreeSet<String>>,
    windows: Mutex<BTreeMap<String, String>>,
}

/// The error a preview raises for a read only a live plan makes.
pub(crate) fn refused(access: &str) -> pyo3::PyErr {
    WarehouseAccessRefusedError::new_err(format!(
        "plan previews read only the synced warehouse state; refused warehouse access: {access}"
    ))
}

fn relation_info<'py>(py: Python<'py>, relation: &StateRelation) -> PyResult<Bound<'py, PyAny>> {
    call_with(
        &host::entry(py, AdapterHost::RelationInfo)?,
        &[
            (
                "database",
                relation.database.as_deref().into_pyobject(py)?.into_any(),
            ),
            ("schema", PyString::new(py, &relation.schema).into_any()),
            ("name", PyString::new(py, &relation.name).into_any()),
            (
                "relation_type",
                PyString::new(py, &relation.relation_type).into_any(),
            ),
        ],
    )
}

fn relation_columns<'py>(
    py: Python<'py>,
    relation: &StateRelation,
) -> PyResult<Bound<'py, PyTuple>> {
    let column_info: Bound<'py, PyAny> = host::entry(py, AdapterHost::ColumnInfo)?;
    let mut columns: Vec<Bound<'py, PyAny>> = Vec::with_capacity(relation.columns.len());
    for column in &relation.columns {
        columns.push(call_with(
            &column_info,
            &[
                ("name", PyString::new(py, &column.name).into_any()),
                ("type", PyString::new(py, &column.data_type).into_any()),
            ],
        )?);
    }
    PyTuple::new(py, columns)
}

fn fingerprint<'py>(
    (class, from_iso): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
    fingerprint: &StateFingerprint,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = class.py();
    let kwargs: Bound<'py, PyDict> = PyDict::new(py);
    kwargs.set_item("node_type", &fingerprint.node_type)?;
    kwargs.set_item("node_name", &fingerprint.node_name)?;
    kwargs.set_item("target_database", &fingerprint.target_database)?;
    kwargs.set_item("target_schema", &fingerprint.target_schema)?;
    kwargs.set_item("target_name", &fingerprint.target_name)?;
    kwargs.set_item("run_id", &fingerprint.run_id)?;
    kwargs.set_item("definition_hash", &fingerprint.definition_hash)?;
    kwargs.set_item("version_hash", &fingerprint.version_hash)?;
    kwargs.set_item("schema_fingerprint", &fingerprint.schema_fingerprint)?;
    kwargs.set_item("definition", &fingerprint.definition)?;
    kwargs.set_item("metadata_json", &fingerprint.metadata_json)?;
    kwargs.set_item(
        "ts",
        from_iso.call1((fingerprint.ts.to_rfc3339_opts(SecondsFormat::AutoSi, true),))?,
    )?;
    class.call((), Some(&kwargs))
}

impl SyncedReads {
    /// Reads over `state`, which several plans may share.
    pub(crate) fn new(state: Arc<WarehouseState>) -> Self {
        Self {
            state,
            unknowns: Mutex::new(BTreeSet::new()),
            windows: Mutex::new(BTreeMap::new()),
        }
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
                .filter(|item| !PREVIEW_UNKNOWNS.contains(&item.as_str()))
                .cloned(),
        );
        unknowns
    }

    /// Each model's window start, read up to the latest data when the run starts.
    pub(crate) fn windows(&self) -> BTreeMap<String, String> {
        self.windows
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
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

    fn listed<'py>(
        &self,
        py: Python<'py>,
        database: &Bound<'py, PyAny>,
        (schemas, names): (Bound<'py, PyAny>, Bound<'py, PyAny>),
    ) -> PyResult<Bound<'py, PyTuple>> {
        let database: Option<String> = database.extract()?;
        let schemas: Option<Vec<String>> = schemas.extract()?;
        let names: Option<Vec<String>> = names.extract()?;
        let relations: Vec<Bound<'py, PyAny>> = self
            .state
            .relations(&RelationFilter {
                database: database.as_deref(),
                schemas: schemas.as_deref(),
                names: names.as_deref(),
            })
            .into_iter()
            .map(|relation| relation_info(py, relation))
            .collect::<PyResult<_>>()?;
        PyTuple::new(py, relations)
    }
}

impl WarehouseReads for SyncedReads {
    fn relations<'py>(
        &self,
        py: Python<'py>,
        database: &Bound<'py, PyAny>,
        filter: (Bound<'py, PyAny>, Bound<'py, PyAny>),
    ) -> PyResult<Bound<'py, PyAny>> {
        Ok(self.listed(py, database, filter)?.into_any())
    }

    fn columns<'py>(
        &self,
        py: Python<'py>,
        relations: Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let found: Bound<'py, PyDict> = PyDict::new(py);
        for relation in relations.try_iter()? {
            let relation: Bound<'py, PyAny> = relation?;
            let database: Option<String> = native_field(&relation, "database")?.extract()?;
            let schema: Option<String> = native_field(&relation, "schema")?.extract()?;
            let name: String = native_field(&relation, "name")?.extract()?;
            let schemas: Option<Vec<String>> = schema.clone().map(|schema| vec![schema]);
            let names: [String; 1] = [name.clone()];
            let listed: Vec<&StateRelation> = self.state.relations(&RelationFilter {
                database: database.as_deref(),
                schemas: schemas.as_deref(),
                names: Some(&names),
            });
            if let Some(listed) = listed.first() {
                found.set_item((database, schema, name), relation_columns(py, listed)?)?;
            }
        }
        Ok(found.into_any())
    }

    fn fingerprints<'py>(
        &self,
        py: Python<'py>,
        read: FingerprintRead<'_>,
    ) -> PyResult<Bound<'py, PyAny>> {
        if !read.table_exists {
            return new_record_with(
                py,
                RecordId::FingerprintSet,
                &[
                    ("schema", PyString::new(py, read.schema).into_any()),
                    ("fingerprints", PyDict::new(py).into_any()),
                ],
            );
        }
        let class: Bound<'py, PyAny> = record_class(py, RecordId::Fingerprint)?;
        let from_iso: Bound<'py, PyAny> = py
            .import("datetime")?
            .getattr("datetime")?
            .getattr("fromisoformat")?;
        let location = StateLocation {
            database: read.database,
            schema: read.schema.to_owned(),
        };
        let latest: Vec<Bound<'py, PyAny>> = self
            .state
            .latest_fingerprints(&location)
            .into_iter()
            .map(|stored| fingerprint((&class, &from_iso), stored))
            .collect::<PyResult<_>>()?;
        selected_set(
            py,
            &location.schema,
            latest,
            (read.node_names, read.filtered_node_types),
        )
    }

    fn within_inspection<'py>(
        &self,
        _py: Python<'py>,
        plan: &mut dyn FnMut() -> PyResult<Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        plan()
    }

    fn inspection_concurrency(&self, _py: Python<'_>) -> PyResult<i64> {
        Ok(1)
    }

    fn prefetch_schema_listings<'py>(
        &self,
        _py: Python<'py>,
        _scopes: Bound<'py, PyAny>,
    ) -> PyResult<()> {
        Ok(())
    }

    fn lists_by_schema(&self, _py: Python<'_>) -> PyResult<bool> {
        Ok(false)
    }

    fn cursor_bounds<'py>(
        &self,
        _py: Python<'py>,
        _statements: Vec<String>,
        _progress: &dyn CursorProgress<'py>,
    ) -> PyResult<Read<Vec<Bound<'py, PyAny>>>> {
        Ok(Read::Unknown(CURSOR_INPUTS.into()))
    }

    fn eligible_cursor_maxes<'py>(
        &self,
        _py: Python<'py>,
        _queries: Vec<EligibleCursorQuery>,
    ) -> PyResult<Read<Vec<Bound<'py, PyAny>>>> {
        Ok(Read::Unknown(CURSOR_INPUTS.into()))
    }

    fn target_max(&self, model: &str) -> Read<String> {
        match self.state.target_max(model) {
            Some(observed) => Read::Known(observed.to_owned()),
            None => Read::Unknown(format!("cursor position of {model}").into()),
        }
    }

    fn note_window(&self, model: &str, start: &str) {
        self.windows
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(model.to_owned(), start.to_owned());
    }

    fn note_unknown(&self, item: String) {
        self.unknowns
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(item);
    }

    fn described_columns(
        &self,
        _py: Python<'_>,
        source: &str,
        sql: &str,
    ) -> PyResult<Read<Vec<String>>> {
        let described = self.state.described_columns(source, sql).map_err(
            |InvalidObservation { source, reason }| {
                PyRuntimeError::new_err(format!(
                    "invalid source_columns observation for {source}: {reason}"
                ))
            },
        )?;
        Ok(match described {
            Some(described) => Read::Known(described),
            None => unknown(
                self,
                format!("query columns of expression source {source} (planned as declared)"),
            ),
        })
    }

    fn read_access<'py>(
        &self,
        _py: Python<'py>,
        _relations: Vec<Bound<'py, PyAny>>,
    ) -> PyResult<Read<Vec<Bound<'py, PyAny>>>> {
        Ok(unknown(self, "read access to these sources"))
    }

    fn source_freshness_state<'py>(
        &self,
        _py: Python<'py>,
        _location: (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
        _read: (bool, Option<&[String]>),
    ) -> PyResult<Read<Bound<'py, PyAny>>> {
        Ok(unknown(self, "source freshness"))
    }

    fn table_freshness<'py>(
        &self,
        py: Python<'py>,
        requests: Bound<'py, PyAny>,
    ) -> PyResult<Read<Bound<'py, PyAny>>> {
        let unavailable =
            host::entry(py, AdapterHost::TableFreshnessMetadata)?.getattr("unavailable")?;
        let outcome = call_with(
            &unavailable,
            &[("message", PyString::new(py, PREVIEW_MESSAGE).into_any())],
        )?;
        let metadata: Bound<'py, PyDict> = PyDict::new(py);
        for request in requests.try_iter()? {
            metadata.set_item(request?, &outcome)?;
        }
        Ok(Read::Known(metadata.into_any()))
    }

    fn single_table_freshness<'py>(
        &self,
        _py: Python<'py>,
        _table: [Bound<'py, PyAny>; 3],
    ) -> PyResult<Read<Bound<'py, PyAny>>> {
        Ok(unknown(self, "source freshness"))
    }

    fn freshness_query<'py>(
        &self,
        _py: Python<'py>,
        _sql: Bound<'py, PyAny>,
    ) -> PyResult<Read<Bound<'py, PyAny>>> {
        Ok(unknown(self, "source freshness"))
    }

    fn table_types(&self) -> Read<()> {
        unknown(self, "table types")
    }

    fn retention<'py>(&self, _py: Python<'py>) -> PyResult<Read<Bound<'py, PyAny>>> {
        Ok(unknown(self, "retention"))
    }

    fn column_rename_unavailable<'py>(
        &self,
        _py: Python<'py>,
        _destination: Bound<'py, PyAny>,
    ) -> PyResult<Read<Bound<'py, PyAny>>> {
        Ok(unknown(self, "column rename support"))
    }

    fn migration_state_tables<'py>(
        &self,
        _py: Python<'py>,
        _database: &Bound<'py, PyAny>,
        _filter: (Bound<'py, PyAny>, Bound<'py, PyAny>),
    ) -> PyResult<Read<Bound<'py, PyAny>>> {
        Ok(unknown(self, "recorded migrations"))
    }

    fn recorded_migrations<'py>(
        &self,
        _py: Python<'py>,
        _layout: &EventTableLayout,
        _location: (Bound<'py, PyAny>, Bound<'py, PyAny>),
        _stored: Option<&[String]>,
    ) -> PyResult<Read<Bound<'py, PyTuple>>> {
        Ok(unknown(self, "recorded migrations"))
    }

    fn old_name_view_columns<'py>(
        &self,
        _py: Python<'py>,
        _location: (Bound<'py, PyAny>, Bound<'py, PyAny>),
    ) -> PyResult<Read<Vec<String>>> {
        Ok(unknown(self, "recorded migrations"))
    }

    fn deferred_relations<'py>(
        &self,
        _py: Python<'py>,
        _location: (Bound<'py, PyAny>, Bound<'py, PyAny>),
    ) -> PyResult<Bound<'py, PyAny>> {
        Err(refused("deferred target relations"))
    }

    fn recent_consumption(
        &self,
        _py: Python<'_>,
        location: &Location,
    ) -> PyResult<Vec<ConsumptionRow>> {
        Ok(recent_executions(
            &self.consumption((location.database.as_deref(), &location.schema)),
            RECENT_EXECUTIONS,
        ))
    }

    fn consumption_executions(
        &self,
        _py: Python<'_>,
        location: &Location,
        execution_ids: &[&str],
    ) -> PyResult<Vec<ConsumptionRow>> {
        let mut rows: Vec<ConsumptionRow> =
            self.consumption((location.database.as_deref(), &location.schema));
        rows.retain(|row| execution_ids.contains(&row.row.execution_id.as_str()));
        Ok(rows)
    }

    fn close_consumption(&self, _py: Python<'_>) -> PyResult<()> {
        Ok(())
    }
}
