//! `(provider, consumer)` for each usage, named by the usage's own `consumer_name` or else by `consumer`.

use crate::discovery::_helpers::sources::provider_secrets::usage_pair;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};

/// `(provider, consumer)` for each usage, named by the usage's own `consumer_name` or else by `consumer`.
pub fn provider_uses(
    usages: &Bound<'_, PyAny>,
    consumer: Option<&str>,
) -> PyResult<Vec<(String, String)>> {
    usages
        .try_iter()?
        .map(|usage| usage_pair(&usage?, consumer))
        .collect()
}
