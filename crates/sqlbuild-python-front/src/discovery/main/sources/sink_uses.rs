//! The providers the project's lifecycle and command-output sinks use, which run with every command.

use crate::discovery::_helpers::sources::provider_secrets::SINK_COLLECTIONS;
use crate::discovery::main::sources::consumer_uses::consumer_uses;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};

/// The providers the project's lifecycle and command-output sinks use, which run with every command.
pub fn sink_uses(discovered: &Bound<'_, PyAny>) -> PyResult<Vec<(String, String)>> {
    let mut uses: Vec<(String, String)> = Vec::new();
    for collection in SINK_COLLECTIONS {
        if let Some(sinks) = discovered.getattr_opt(collection)? {
            uses.extend(consumer_uses(&sinks)?);
        }
    }
    Ok(uses)
}
