//! A preview's answers to the planner's warehouse reads, from the synced state's indexes.

use chrono::SecondsFormat;
use pyo3::IntoPyObject;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods, PyString, PyTuple};
use sqlbuild_host::entries::models::AdapterHost;
use sqlbuild_state::warehouse_state::models::{
    RelationFilter, StateFingerprint, StateLocation, StateRelation,
};

use crate::bindings::_helpers::boundary::python_calls::call_with;
use crate::bindings::_helpers::fingerprints::latest_rows::selected_set;
use crate::bindings::_helpers::native_host::host;
use crate::bindings::_helpers::native_host::preview_state::{PREVIEW_MESSAGE, PreviewConnection};
use crate::bindings::_helpers::records::construct::record_class;
use crate::bindings::_helpers::records::fields::field as native_field;
use crate::bindings::_helpers::records::ids::RecordId;

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

fn columns<'py>(py: Python<'py>, relation: &StateRelation) -> PyResult<Bound<'py, PyTuple>> {
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

/// The listed relations of the synced schemas that `filter` names, as `RelationInfo`s.
pub(crate) fn preview_relations<'py>(
    preview: &Bound<'py, PreviewConnection>,
    filter: &RelationFilter<'_>,
) -> PyResult<Bound<'py, PyTuple>> {
    let py: Python<'py> = preview.py();
    let relations: Vec<Bound<'py, PyAny>> = preview
        .get()
        .state()
        .relations(filter)
        .into_iter()
        .map(|relation| relation_info(py, relation))
        .collect::<PyResult<_>>()?;
    PyTuple::new(py, relations)
}

/// The synced columns of each listed relation, keyed by `(database, schema, name)` as requested.
pub(crate) fn preview_columns<'py>(
    preview: &Bound<'py, PreviewConnection>,
    relations: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyDict>> {
    let py: Python<'py> = preview.py();
    let found: Bound<'py, PyDict> = PyDict::new(py);
    for relation in relations.try_iter()? {
        let relation: Bound<'py, PyAny> = relation?;
        let database: Option<String> = native_field(&relation, "database")?.extract()?;
        let schema: Option<String> = native_field(&relation, "schema")?.extract()?;
        let name: String = native_field(&relation, "name")?.extract()?;
        let schemas: Option<Vec<String>> = schema.clone().map(|schema| vec![schema]);
        let names: [String; 1] = [name.clone()];
        let listed: Vec<&StateRelation> = preview.get().state().relations(&RelationFilter {
            database: database.as_deref(),
            schemas: schemas.as_deref(),
            names: Some(&names),
        });
        if let Some(listed) = listed.first() {
            found.set_item((database, schema, name), columns(py, listed)?)?;
        }
    }
    Ok(found)
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

/// The newest fingerprint per node of one synced state location, narrowed as a filtered read would be.
pub(crate) fn preview_fingerprints<'py>(
    preview: &Bound<'py, PreviewConnection>,
    location: &StateLocation,
    (node_names, filtered_node_types): (Option<Vec<String>>, Vec<String>),
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = preview.py();
    let class: Bound<'py, PyAny> = record_class(py, RecordId::Fingerprint)?;
    let from_iso: Bound<'py, PyAny> = py
        .import("datetime")?
        .getattr("datetime")?
        .getattr("fromisoformat")?;
    let latest: Vec<Bound<'py, PyAny>> = preview
        .get()
        .state()
        .latest_fingerprints(location)
        .into_iter()
        .map(|stored| fingerprint((&class, &from_iso), stored))
        .collect::<PyResult<_>>()?;
    selected_set(
        py,
        &location.schema,
        latest,
        (node_names, filtered_node_types),
    )
}

/// Each requested table's freshness metadata as unavailable: the state holds past observations only.
pub(crate) fn preview_freshness_metadata<'py>(
    py: Python<'py>,
    requests: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyDict>> {
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
    Ok(metadata)
}
