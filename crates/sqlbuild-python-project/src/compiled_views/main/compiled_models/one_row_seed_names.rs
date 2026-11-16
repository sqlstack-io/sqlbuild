//! Names of the seeds whose CSV holds at most one data row, from `seed_file` and `schema_entry`.

use crate::compiled_views::_helpers::compiled_models::relation_keys;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// Names of the seeds whose CSV holds at most one data row, from `seed_file` and `schema_entry`.
#[inline]
pub fn one_row_seed_names(seeds: &[Bound<'_, PyAny>]) -> PyResult<Vec<String>> {
    relation_keys::one_row_seed_names(seeds)
}
