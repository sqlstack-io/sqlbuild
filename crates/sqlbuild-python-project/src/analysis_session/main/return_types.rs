//! `profile.function_return_types` items in order.

use crate::analysis_session::_helpers::profile_inputs::pairs;
use pyo3::prelude::{Bound, PyAny, PyResult};
use sqlbuild_analysis::assembly::analysis_session::types::Pairs;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// `profile.function_return_types` items in order.
pub fn return_types(profile: &Bound<'_, PyAny>) -> PyResult<Pairs> {
    pairs(&native_field(profile, "function_return_types")?)
}
