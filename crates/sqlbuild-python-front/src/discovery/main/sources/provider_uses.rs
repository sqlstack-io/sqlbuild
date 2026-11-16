//! `(provider, consumer)` for each usage, named by the usage's own `consumer_name` or else by `consumer`.

use crate::discovery::_helpers::sources::provider_secrets;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// `(provider, consumer)` for each usage, named by the usage's own `consumer_name` or else by `consumer`.
#[inline]
pub fn provider_uses(
    usages: &Bound<'_, PyAny>,
    consumer: Option<&str>,
) -> PyResult<Vec<(String, String)>> {
    provider_secrets::provider_uses(usages, consumer)
}
