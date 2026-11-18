//! Warehouse reads answered from synced state, refusing what it cannot answer.

use crate::planner_strategy::main::model_entries::column_info::column_info;
use crate::planner_warehouse::_helpers::synced::exception_types::WarehouseAccessRefusedError;
use chrono::SecondsFormat;
use pyo3::IntoPyObject;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods, PyString, PyTuple};
use sqlbuild_host::entries::models::AdapterHost;
use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::native_host::main::host_entry::host_entry;
use sqlbuild_python_base::records::main::construct::new_record::new_record;
use sqlbuild_python_base::records::types::RecordId;
use sqlbuild_state::warehouse_state::models::{StateFingerprint, StateRelation};

/// The error a preview raises for a read only a live plan makes.
pub(crate) fn refused(access: &str) -> pyo3::PyErr {
    WarehouseAccessRefusedError::new_err(format!(
        "plan previews read only the synced warehouse state; refused warehouse access: {access}"
    ))
}

pub(crate) fn relation_info<'py>(
    py: Python<'py>,
    relation: &StateRelation,
) -> PyResult<Bound<'py, PyAny>> {
    call_with(
        &host_entry(py, AdapterHost::RelationInfo)?,
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

pub(crate) fn relation_columns<'py>(
    py: Python<'py>,
    relation: &StateRelation,
) -> PyResult<Bound<'py, PyTuple>> {
    let class: Bound<'py, PyAny> = host_entry(py, AdapterHost::ColumnInfo)?;
    let columns: Vec<Bound<'py, PyAny>> = relation
        .columns
        .iter()
        .map(|column| {
            column_info(
                &class,
                (
                    PyString::new(py, &column.name).into_any(),
                    PyString::new(py, &column.data_type).into_any(),
                ),
            )
        })
        .collect::<PyResult<_>>()?;
    PyTuple::new(py, columns)
}

pub(crate) fn fingerprint<'py>(
    datetime: &Bound<'py, PyAny>,
    fingerprint: &StateFingerprint,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = datetime.py();
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
        datetime.call_method1(
            "fromisoformat",
            (fingerprint.ts.to_rfc3339_opts(SecondsFormat::AutoSi, true),),
        )?,
    )?;
    new_record(py, RecordId::Fingerprint, (), Some(&kwargs))
}
