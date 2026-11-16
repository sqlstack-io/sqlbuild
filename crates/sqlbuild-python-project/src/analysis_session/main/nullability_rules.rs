//! The adapter nullability rules as `(name, rule id)` rows, and the adapter's own rules.

use crate::analysis_session::_helpers::profile_inputs;
use crate::analysis_session::_helpers::profile_inputs::AdapterRules;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use sqlbuild_analysis::assembly::analysis_session::types::Pairs;

/// The adapter nullability rules as `(name, rule id)` rows, and the adapter's own rules.
#[inline]
pub fn nullability_rules(profile: &Bound<'_, PyAny>) -> PyResult<(Pairs, AdapterRules)> {
    profile_inputs::nullability_rules(profile)
}
