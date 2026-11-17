//! Fixture-column inference: columns plus explicitly untyped NULL projections.

use std::collections::HashMap;

use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::pyfunction;
use pyo3::sync::PyOnceLock;
use pyo3::types::{PyDict, PyDictMethods, PyFrozenSet, PyTuple};
use sqlbuild_analysis::assembly::analysis_session::main::projection_facts::projection_facts;
use sqlbuild_analysis::assembly::analysis_session::models::ProjectionFacts;
use sqlbuild_core::panics::main::native_failure::native_failure;
use sqlbuild_core::text::main::python_casefold::python_casefold;

use crate::analysis_session::_helpers::stage_functions::query_column_rows;
use crate::analysis_session::models::FixtureInferenceCount;
use crate::analysis_session::types::ColumnsRow;
use sqlbuild_python_base::boundary::main::compiler_error::compiler_error;
use sqlbuild_python_base::boundary::models::Request;
use sqlbuild_python_base::records::main::construct::enum_from_value::enum_from_value;
use sqlbuild_python_base::records::main::construct::new_record::new_record;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::RecordId;
use sqlbuild_python_base::types::CompilerDetach;

const PROJECTION_FACTS_CONTEXT: &str = "native fixture projection facts";
const LEGACY_MODE: &str = "legacy";

/// Fixture column inferences computed in this process.
static INFERENCES: PyOnceLock<Py<FixtureInferenceCount>> = PyOnceLock::new();

/// `infer_fixture_column_facts(*, query_sql, inference_profile)`: a `FixtureColumnInference`.
#[pyfunction]
#[pyo3(signature = (**request))]
pub(crate) fn infer_fixture_column_facts<'py>(
    request: Option<&Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    let request = Request::new("infer_fixture_column_facts", request)?;
    let py: Python<'py> = request.py();
    inference_count(py)?.borrow_mut().computed += 1;
    let query_sql: String = request.get("query_sql")?.extract()?;
    let profile: Bound<'py, PyAny> = request.get("inference_profile")?;
    let query: Bound<'py, PyDict> = PyDict::new(py);
    query.set_item("sql", &query_sql)?;
    query.set_item("mode", LEGACY_MODE)?;
    query.set_item("recover_cte_facts", false)?;
    let query: Bound<'py, PyAny> = new_record(py, RecordId::NativeColumnQuery, (), Some(&query))?;
    let empty: Bound<'py, PyAny> = PyDict::new(py).into_any();
    let rows: Vec<ColumnsRow> = query_column_rows(
        PyTuple::new(py, [query])?.as_any(),
        &profile,
        (&empty, &empty),
    )?;
    let Some((true, Some(columns), false)) = rows.into_iter().next() else {
        return Ok(py.None().into_bound(py));
    };
    let dialect: Option<String> = native_field(&profile, "sql_analysis_dialect")?.extract()?;
    let facts: ProjectionFacts = py
        .compiler_detach(|| projection_facts(&query_sql, dialect.as_deref()))
        .map_err(|reason| compiler_error(native_failure(PROJECTION_FACTS_CONTEXT, &reason)))?;
    let simple_types: HashMap<String, String> = facts.types.into_iter().collect();
    let mut built: Vec<Bound<'py, PyAny>> = Vec::with_capacity(columns.len());
    for (name, data_type, nullability) in columns {
        let data_type: Option<String> = data_type
            .filter(|data_type| !data_type.is_empty())
            .or_else(|| simple_types.get(&python_casefold(&name)).cloned());
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
    let arguments: Bound<'py, PyDict> = PyDict::new(py);
    arguments.set_item("columns", PyTuple::new(py, built)?)?;
    arguments.set_item(
        "null_literal_names",
        PyFrozenSet::new(py, facts.null_literal_names)?,
    )?;
    arguments.set_item("quoted_names", PyFrozenSet::new(py, facts.quoted_names)?)?;
    new_record(py, RecordId::FixtureColumnInference, (), Some(&arguments))
}

/// Fixture column inferences computed in this process, optionally clearing the count first.
#[pyfunction]
#[pyo3(signature = (*, reset = false))]
pub(crate) fn fixture_inference_count(py: Python<'_>, reset: bool) -> PyResult<u64> {
    let count = inference_count(py)?;
    let mut count = count.borrow_mut();
    if reset {
        count.computed = 0;
    }
    Ok(count.computed)
}

/// The extension-wide fixture inference counter.
fn inference_count(py: Python<'_>) -> PyResult<Bound<'_, FixtureInferenceCount>> {
    Ok(INFERENCES
        .get_or_try_init(py, || Py::new(py, FixtureInferenceCount::default()))?
        .bind(py)
        .clone())
}
