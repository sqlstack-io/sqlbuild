//! `profile.sql_analysis_dialect or "generic"`.

use crate::analysis_session::main::dialect_or_generic::dialect_or_generic;
use pyo3::prelude::{Bound, PyAny, PyResult};
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// `profile.sql_analysis_dialect or "generic"`.
pub fn profile_dialect(profile: &Bound<'_, PyAny>) -> PyResult<String> {
    dialect_or_generic(&native_field(profile, "sql_analysis_dialect")?)
}
