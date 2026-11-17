//! `(provider, consumer)` for each provider the named consumers, such as checks or loaders, inject.

use crate::discovery::main::sources::provider_uses::provider_uses;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_python_base::records::main::fields::field::field;

/// `(provider, consumer)` for each provider the named consumers, such as checks or loaders, inject.
pub fn consumer_uses(consumers: &Bound<'_, PyAny>) -> PyResult<Vec<(String, String)>> {
    let mut uses: Vec<(String, String)> = Vec::new();
    for consumer in consumers.try_iter()? {
        let consumer: Bound<'_, PyAny> = consumer?;
        let name: String = field(&consumer, "name")?.extract()?;
        uses.extend(provider_uses(
            &field(&consumer, "provider_usages")?,
            Some(&name),
        )?);
    }
    Ok(uses)
}
