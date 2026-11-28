//! Name a macro by its name, file, identity and exact source, or None when it has no key.

use crate::macro_bridge::_helpers::store_keys::CanonicalTypes;
use crate::macro_bridge::_helpers::store_keys::unstable;
use crate::macro_bridge::main::text::text;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::{PyDict, PyString};
use sqlbuild_cache::macro_store_keys::main::macro_store_token::macro_store_token as macro_key;
use sqlbuild_core::json::models::JsonValue;
use sqlbuild_python_base::boundary::models::KeywordArguments;
use sqlbuild_python_base::frontier::main::emit_error::emit_error;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// Name a macro by its name, file, identity and exact source, or None when it has no key.
#[pyfunction]
#[pyo3(signature = (**options))]
pub fn macro_store_token<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<Option<String>> {
    let arguments: KeywordArguments<'py> = KeywordArguments::read(
        py,
        "macro_store_token",
        options,
        (&["loaded", "identity"], &[]),
    )?;
    let loaded: Bound<'py, PyAny> = arguments.value("loaded")?;
    let keyed: PyResult<Option<String>> = (|| {
        let types: CanonicalTypes<'py> = CanonicalTypes::load(py)?;
        let name: JsonValue = text(native_field(&loaded, "name")?.cast::<PyString>()?)?;
        let path: JsonValue = text(
            native_field(&loaded, "relative_path")?
                .call_method0("as_posix")?
                .cast::<PyString>()?,
        )?;
        let identity: JsonValue = types.canonical(&arguments.value("identity")?, 0)?;
        let raw_source: String = native_field(&loaded, "raw_source")?.extract()?;
        macro_key((name, path), identity, &raw_source).map_err(emit_error)
    })();
    match keyed {
        Err(error) if unstable(py, &error) => Ok(None),
        other => other,
    }
}
