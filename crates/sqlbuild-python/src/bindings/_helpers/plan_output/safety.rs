//! Structured future-cursor and maximum-start safety evidence for plan and result JSON.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyModule, PyModuleMethods, PyResult, Python};
use pyo3::types::{PyDict, PyString};
use pyo3::{pyfunction, wrap_pyfunction};

use crate::bindings::_helpers::cursor::scalars::render_value;
use crate::bindings::_helpers::plan_output::values::{dict, enum_value, items};
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// `render(value=value)`.
fn render<'py>(value: Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    Ok(PyString::new(value.py(), &render_value(&value)?).into_any())
}

/// `render(value=value)` when set, else `None`.
fn optional_render<'py>(value: Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    if value.is_none() {
        return Ok(value);
    }
    render(value)
}

fn bounds<'py>(
    evidence: &Bound<'py, PyAny>,
    (start, end): (&str, &str),
) -> PyResult<Bound<'py, PyAny>> {
    Ok(dict(
        evidence.py(),
        vec![
            ("start", render(native_field(evidence, start)?)?),
            ("end", render(native_field(evidence, end)?)?),
        ],
    )?
    .into_any())
}

/// Cap policy, bounds and physical input evidence of future-cursor safety, or `None`.
pub(crate) fn future_cursor_safety<'py>(
    evidence: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = evidence.py();
    if evidence.is_none() {
        return Ok(evidence.clone());
    }
    let relation: Bound<'py, PyAny> = native_field(evidence, "determining_relation")?;
    let determining: Bound<'py, PyAny> = if relation.is_none() {
        relation
    } else {
        dict(
            py,
            vec![
                ("relation", relation),
                (
                    "cursor_column",
                    native_field(evidence, "determining_cursor_column")?,
                ),
            ],
        )?
        .into_any()
    };
    let mut inputs: Vec<Bound<'py, PyDict>> = Vec::new();
    for item in items(&native_field(evidence, "inputs")?)? {
        inputs.push(dict(
            py,
            vec![
                ("relation", native_field(&item, "relation")?),
                ("cursor_column", native_field(&item, "cursor_column")?),
                ("minimum", optional_render(native_field(&item, "minimum")?)?),
                ("maximum", render(native_field(&item, "maximum")?)?),
            ],
        )?);
    }
    Ok(dict(
        py,
        vec![
            ("action", enum_value(evidence, "action")?),
            ("max_distance", native_field(evidence, "max_distance")?),
            (
                "invocation_time",
                render(native_field(evidence, "invocation_time")?)?,
            ),
            (
                "discovered_bounds",
                bounds(evidence, ("discovered_start", "discovered_end"))?,
            ),
            (
                "applied_bounds",
                bounds(evidence, ("applied_start", "applied_end"))?,
            ),
            (
                "maximum_allowed_bounds",
                bounds(evidence, ("maximum_allowed_start", "maximum_allowed_end"))?,
            ),
            (
                "future_start_detected",
                native_field(evidence, "future_start_detected")?,
            ),
            (
                "future_end_detected",
                native_field(evidence, "future_end_detected")?,
            ),
            ("determining_input", determining),
            ("inputs", pyo3::types::PyList::new(py, inputs)?.into_any()),
        ],
    )?
    .into_any())
}

/// Automatic-start policy and physical eligibility evidence, or `None`.
pub(crate) fn maximum_start_safety<'py>(
    evidence: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = evidence.py();
    if evidence.is_none() {
        return Ok(evidence.clone());
    }
    Ok(dict(
        py,
        vec![
            ("action", enum_value(evidence, "action")?),
            ("max_ahead", native_field(evidence, "max_ahead")?),
            (
                "invocation_time",
                render(native_field(evidence, "invocation_time")?)?,
            ),
            (
                "physical_target_max",
                render(native_field(evidence, "physical_target_max")?)?,
            ),
            (
                "highest_eligible_target_max",
                optional_render(native_field(evidence, "highest_eligible_target_max")?)?,
            ),
            (
                "effective_start",
                render(native_field(evidence, "effective_start")?)?,
            ),
            (
                "maximum_allowed_start",
                render(native_field(evidence, "maximum_allowed_start")?)?,
            ),
            (
                "input",
                dict(
                    py,
                    vec![
                        ("relation", native_field(evidence, "target_relation")?),
                        ("cursor_column", native_field(evidence, "cursor_column")?),
                    ],
                )?
                .into_any(),
            ),
        ],
    )?
    .into_any())
}

/// Serialize maximum automatic-start safety evidence; `None` stays `None`.
#[pyfunction]
fn serialize_maximum_start_safety<'py>(evidence: Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    maximum_start_safety(&evidence)
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(serialize_maximum_start_safety, module)?)
}
