//! dbt node results as the Python `DbtNodeExecutionResult` model.

use pyo3::BoundObject;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyString, PyTuple};
use sqlbuild_core::dbt_events::models::{DbtNodeMessage, DbtNodeResult};

use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::RecordId;

fn optional<'py, T: pyo3::IntoPyObject<'py>>(
    py: Python<'py>,
    value: Option<T>,
) -> PyResult<Bound<'py, PyAny>> {
    match value {
        Some(value) => Ok(value
            .into_pyobject(py)
            .map_err(Into::into)?
            .into_any()
            .into_bound()),
        None => Ok(py.None().into_bound(py)),
    }
}

fn text<'py>(py: Python<'py>, value: &str) -> Bound<'py, PyAny> {
    PyString::new(py, value).into_any()
}

/// A Python `DbtNodeExecutionResult`.
pub(crate) fn python_result<'py>(
    py: Python<'py>,
    result: &DbtNodeResult,
) -> PyResult<Bound<'py, PyAny>> {
    let messages: Vec<Bound<'py, PyAny>> = result
        .messages
        .iter()
        .map(|message| {
            new_record_with(
                py,
                RecordId::DbtNodeMessage,
                &[
                    ("level", text(py, &message.level)),
                    ("message", text(py, &message.message)),
                ],
            )
        })
        .collect::<PyResult<_>>()?;
    new_record_with(
        py,
        RecordId::DbtNodeExecutionResult,
        &[
            ("unique_id", text(py, &result.unique_id)),
            ("resource_type", text(py, &result.resource_type)),
            ("node_name", text(py, &result.node_name)),
            ("status", text(py, &result.status)),
            ("index", optional(py, result.index)?),
            ("total", optional(py, result.total)?),
            ("execution_time", optional(py, result.execution_time)?),
            (
                "materialized",
                optional(py, result.materialized.as_deref())?,
            ),
            (
                "relation_name",
                optional(py, result.relation_name.as_deref())?,
            ),
            ("database", optional(py, result.database.as_deref())?),
            ("schema", optional(py, result.schema.as_deref())?),
            (
                "node_checksum",
                optional(py, result.node_checksum.as_deref())?,
            ),
            ("messages", PyTuple::new(py, messages)?.into_any()),
        ],
    )
}

fn field<T: for<'a> pyo3::FromPyObject<'a>>(value: &Bound<'_, PyAny>, name: &str) -> PyResult<T> {
    native_field(value, name)?.extract()
}

/// A Python `DbtNodeExecutionResult` as plain data.
pub(crate) fn native_result(result: &Bound<'_, PyAny>) -> PyResult<DbtNodeResult> {
    let messages: Vec<DbtNodeMessage> = native_field(result, "messages")?
        .try_iter()?
        .map(|message| {
            let message: Bound<'_, PyAny> = message?;
            Ok(DbtNodeMessage {
                level: field(&message, "level")?,
                message: field(&message, "message")?,
            })
        })
        .collect::<PyResult<_>>()?;
    Ok(DbtNodeResult {
        unique_id: field(result, "unique_id")?,
        resource_type: field(result, "resource_type")?,
        node_name: field(result, "node_name")?,
        status: field(result, "status")?,
        index: field(result, "index")?,
        total: field(result, "total")?,
        execution_time: field(result, "execution_time")?,
        materialized: field(result, "materialized")?,
        relation_name: field(result, "relation_name")?,
        database: field(result, "database")?,
        schema: field(result, "schema")?,
        node_checksum: field(result, "node_checksum")?,
        messages,
    })
}
