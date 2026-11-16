//! Unknown and unsupported authored keys, rejected with nearest-key suggestions natively.

use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::{pyfunction, wrap_pyfunction};
use sqlbuild_discovery::key_checks::main::unknown_mapping_keys::unknown_mapping_keys as unknown_keys;
use sqlbuild_discovery::key_checks::main::unsupported_keys_help::unsupported_keys_help as keys_help;

/// `(message after the label, help)` when a mapping key is outside `allowed`.
#[pyfunction]
fn unknown_mapping_keys(
    keys: Vec<(String, bool)>,
    allowed: Vec<String>,
) -> Option<(String, String)> {
    unknown_keys(&keys, &allowed)
}

/// `unsupported_keys_help` for `keys` outside `supported_keys`.
#[pyfunction]
fn unsupported_keys_help(keys: Vec<String>, supported_keys: Vec<String>) -> String {
    let keys: Vec<&str> = keys.iter().map(String::as_str).collect();
    keys_help(&keys, &supported_keys)
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(unknown_mapping_keys, module)?)?;
    module.add_function(wrap_pyfunction!(unsupported_keys_help, module)?)
}
