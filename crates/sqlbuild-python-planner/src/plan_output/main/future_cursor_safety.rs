//! Cap policy, bounds and physical input evidence of future-cursor safety, or `None`.

use crate::plan_output::_helpers::safety::bounds;
use crate::plan_output::_helpers::safety::optional_render;
use crate::plan_output::_helpers::safety::render;
use crate::plan_output::_helpers::values::enum_value;
use crate::plan_output::main::dict::dict;
use crate::plan_output::main::items::items;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::PyDict;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// Cap policy, bounds and physical input evidence of future-cursor safety, or `None`.
pub fn future_cursor_safety<'py>(evidence: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
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
