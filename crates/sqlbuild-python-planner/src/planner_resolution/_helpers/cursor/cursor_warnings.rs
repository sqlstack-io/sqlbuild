//! Prominent warnings for applied future-end and automatic-start cursor safety.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use pyo3::pyfunction;
use pyo3::types::PyString;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_front::cursor::main::scalars::render_value::render_value;

pub(crate) const FUTURE_CAPPED_CODE: &str = "FUTURE_CURSOR_CAPPED";
pub(crate) const START_CAPPED_CODE: &str = "AUTOMATIC_START_CAPPED";

fn rendered(evidence: &Bound<'_, PyAny>, field: &str) -> PyResult<String> {
    render_value(&native_field(evidence, field)?)
}

/// Render a prominent warning for applied future cursor policy.
pub(crate) fn future_cap_message(bounds: &Bound<'_, PyAny>) -> PyResult<Option<String>> {
    if bounds.is_none() {
        return Ok(None);
    }
    let evidence = native_field(bounds, "future_safety")?;
    if evidence.is_none() {
        return Ok(None);
    }
    Ok(Some(format!(
        "FUTURE CURSOR CAPPED: discovered bounds ['{}', '{}'), applied bounds ['{}', '{}').",
        rendered(&evidence, "discovered_start")?,
        rendered(&evidence, "discovered_end")?,
        rendered(&evidence, "applied_start")?,
        rendered(&evidence, "applied_end")?,
    )))
}

/// Render a warning that distinguishes start recovery from future-end capping.
pub(crate) fn maximum_start_message(bounds: &Bound<'_, PyAny>) -> PyResult<Option<String>> {
    if bounds.is_none() {
        return Ok(None);
    }
    let evidence = native_field(bounds, "maximum_start_safety")?;
    if evidence.is_none() {
        return Ok(None);
    }
    let highest = native_field(&evidence, "highest_eligible_target_max")?;
    let highest = if highest.is_none() {
        "None".to_owned()
    } else {
        PyString::new(bounds.py(), &render_value(&highest)?)
            .repr()?
            .extract()?
    };
    Ok(Some(format!(
        "AUTOMATIC START CAPPED: physical target MAX '{}' exceeded horizon '{}'; highest \
         eligible target MAX={highest}, effective post-lookback start='{}', action={}, \
         input={}.{}.",
        rendered(&evidence, "physical_target_max")?,
        rendered(&evidence, "maximum_allowed_start")?,
        rendered(&evidence, "effective_start")?,
        native_field(&native_field(&evidence, "action")?, "value")?.str()?,
        native_field(&evidence, "target_relation")?.str()?,
        native_field(&evidence, "cursor_column")?.str()?,
    )))
}

/// Render a prominent warning for applied future cursor policy.
#[pyfunction]
#[pyo3(signature = (bounds, /))]
pub(crate) fn future_cursor_cap_warning(bounds: &Bound<'_, PyAny>) -> PyResult<Option<String>> {
    future_cap_message(bounds)
}

/// Render a warning that distinguishes start recovery from future-end capping.
#[pyfunction]
#[pyo3(signature = (bounds, /))]
pub(crate) fn maximum_start_cap_warning(bounds: &Bound<'_, PyAny>) -> PyResult<Option<String>> {
    maximum_start_message(bounds)
}
