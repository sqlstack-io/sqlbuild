//! Lineage keys of every Python key in `keys`.

use crate::lineage::main::lineage_key::lineage_key;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_analysis::lineage_command::models::LineageKey;

/// Lineage keys of every Python key in `keys`.
pub fn lineage_keys(keys: &Bound<'_, PyAny>) -> PyResult<Vec<LineageKey>> {
    keys.try_iter()?.map(|key| lineage_key(&key?)).collect()
}
