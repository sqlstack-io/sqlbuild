//! Observe the selected sources inside the run's observability scope.

use crate::freshness::_helpers::freshness_observe;
use crate::freshness::models::Observer;
use crate::freshness::models::PreviousState;
use pyo3::prelude::{Bound, PyAny, PyResult};
use sqlbuild_core::freshness_text::models::FreshnessSourceResult;

/// Observe the selected sources inside the run's observability scope.
#[inline]
pub fn observe_freshness(
    observer: &Observer<'_>,
    (sources, select, exclude): (&Bound<'_, PyAny>, &[String], &[String]),
    (run_id, state): (Option<String>, &PreviousState<'_>),
) -> PyResult<Vec<FreshnessSourceResult>> {
    freshness_observe::observe_freshness(observer, (sources, select, exclude), (run_id, state))
}
