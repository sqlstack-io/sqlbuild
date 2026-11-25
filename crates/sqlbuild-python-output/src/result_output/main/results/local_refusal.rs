//! A local run's failure where the warehouse would not fail, as a result's `local_refusal` holds it.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_core::run_on::main::read_local_refusal::read_local_refusal;
use sqlbuild_core::run_on::models::LocalRefusal;
use std::collections::HashMap;

/// The item named in a `local_refusal` value and its refusal; `None` when the value is `None`.
pub fn local_refusal(value: &Bound<'_, PyAny>) -> PyResult<Option<(String, LocalRefusal)>> {
    let found: Option<HashMap<String, String>> = value.extract()?;
    Ok(found.map(read_local_refusal))
}
