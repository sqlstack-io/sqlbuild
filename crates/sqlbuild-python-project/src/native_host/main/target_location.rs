//! A target's database or schema as a location, unless it is `preserve`.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_analysis::assembly::target_checks::constants::PRESERVE_TARGET_VALUE;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// The active target's database or schema as a location: `preserve` names none, since each model keeps its own.
pub fn target_location(project: &Bound<'_, PyAny>, field: &str) -> PyResult<Option<String>> {
    Ok(native_field(project, field)?
        .extract::<Option<String>>()?
        .filter(|value| value != PRESERVE_TARGET_VALUE))
}
