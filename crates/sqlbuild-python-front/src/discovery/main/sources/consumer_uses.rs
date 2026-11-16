//! `(provider, consumer)` for each provider the named consumers, such as checks or loaders, inject.

use crate::discovery::_helpers::sources::provider_secrets;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// `(provider, consumer)` for each provider the named consumers, such as checks or loaders, inject.
#[inline]
pub fn consumer_uses(consumers: &Bound<'_, PyAny>) -> PyResult<Vec<(String, String)>> {
    provider_secrets::consumer_uses(consumers)
}
