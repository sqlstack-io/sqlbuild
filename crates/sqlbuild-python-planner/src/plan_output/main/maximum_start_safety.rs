//! Automatic-start policy and physical eligibility evidence, or `None`.

use crate::plan_output::_helpers::safety::optional_render;
use crate::plan_output::_helpers::safety::render;
use crate::plan_output::_helpers::values::enum_value;
use crate::plan_output::main::dict::dict;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// Automatic-start policy and physical eligibility evidence, or `None`.
pub fn maximum_start_safety<'py>(evidence: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
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
