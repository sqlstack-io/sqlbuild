//! Each query's columns as `NativeQueryColumns`; a native internal failure raises.

use crate::analysis_session::_helpers::stage_functions::or_empty;
use crate::analysis_session::_helpers::stage_functions::query_column_rows;
use crate::analysis_session::types::ColumnsRow;
use pyo3::prelude::{Bound, PyAny, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::{PyDict, PyDictMethods, PyTuple};
use sqlbuild_python_base::boundary::models::Request;
use sqlbuild_python_base::records::main::construct::enum_from_value::enum_from_value;
use sqlbuild_python_base::records::main::construct::new_record::new_record;
use sqlbuild_python_base::records::types::RecordId;

/// Each query's columns as `NativeQueryColumns`; a native internal failure raises.
#[pyfunction]
#[pyo3(signature = (**request))]
pub fn infer_native_query_columns<'py>(
    request: Option<&Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyTuple>> {
    let request = Request::new("infer_native_query_columns", request)?;
    let py: Python<'py> = request.py();
    let empty: Bound<'py, PyAny> = PyDict::new(py).into_any();
    let types: Bound<'py, PyAny> = or_empty(request.optional("column_types_by_table")?, &empty)?;
    let nullability: Bound<'py, PyAny> =
        or_empty(request.optional("column_nullability_by_table")?, &empty)?;
    let rows: Vec<ColumnsRow> = query_column_rows(
        &request.get("queries")?,
        &request.get("profile")?,
        (&types, &nullability),
    )?;
    let mut analyses: Vec<Bound<'py, PyAny>> = Vec::with_capacity(rows.len());
    for (succeeded, columns, has_star) in rows {
        let columns: Bound<'py, PyAny> = match columns {
            None => py.None().into_bound(py),
            Some(columns) => {
                let mut built: Vec<Bound<'py, PyAny>> = Vec::with_capacity(columns.len());
                for (name, data_type, nullability) in columns {
                    let arguments: Bound<'py, PyDict> = PyDict::new(py);
                    arguments.set_item("name", name)?;
                    arguments.set_item("type", data_type)?;
                    arguments.set_item(
                        "nullability",
                        enum_from_value(py, RecordId::InferredNullability, nullability)?,
                    )?;
                    built.push(new_record(
                        py,
                        RecordId::InferredColumn,
                        (),
                        Some(&arguments),
                    )?);
                }
                PyTuple::new(py, built)?.into_any()
            }
        };
        let arguments: Bound<'py, PyDict> = PyDict::new(py);
        arguments.set_item("succeeded", succeeded)?;
        arguments.set_item("columns", columns)?;
        arguments.set_item("has_star", has_star)?;
        analyses.push(new_record(
            py,
            RecordId::NativeQueryColumns,
            (),
            Some(&arguments),
        )?);
    }
    PyTuple::new(py, analyses)
}
