//! Digest the whole context a macro may read, or None when a value has no stable text.

use crate::macro_bridge::_helpers::store_keys::CanonicalTypes;
use crate::macro_bridge::_helpers::store_keys::canonical_context;
use crate::macro_bridge::_helpers::store_keys::plain_json;
use crate::macro_bridge::_helpers::store_keys::unstable;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::PyDict;
use sqlbuild_cache::macro_store_keys::main::context_store_token::context_store_token as context_key;
use sqlbuild_core::json::models::JsonValue;
use sqlbuild_python_base::boundary::main::imported::imported;
use sqlbuild_python_base::boundary::models::KeywordArguments;
use sqlbuild_python_base::frontier::main::emit_error::emit_error;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// Digest the whole context a macro may read, or None when a value has no stable text.
#[pyfunction]
#[pyo3(signature = (**options))]
pub fn context_store_token<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<Option<String>> {
    let arguments: KeywordArguments<'py> = KeywordArguments::read(
        py,
        "context_store_token",
        options,
        (&["macro_context", "declarations"], &[]),
    )?;
    let keyed: PyResult<Option<String>> = (|| {
        let types: CanonicalTypes<'py> = CanonicalTypes::load(py)?;
        let context: JsonValue = canonical_context(&types, &arguments.value("macro_context")?)?;
        let declarations: Bound<'py, PyAny> = arguments.value("declarations")?;
        let declared: JsonValue = if declarations.is_none() {
            JsonValue::Null
        } else {
            let sorted = imported(py, "builtins", "sorted")?;
            let mut parts: Vec<JsonValue> = vec![
                types.canonical(&native_field(&declarations, "constants")?, 0)?,
                types.canonical(&native_field(&declarations, "enums")?, 0)?,
            ];
            for name in ["inaccessible_constants", "inaccessible_enums"] {
                let mut items: Vec<JsonValue> = Vec::new();
                for item in sorted
                    .call1((native_field(&declarations, name)?,))?
                    .try_iter()?
                {
                    items.push(plain_json(&item?)?);
                }
                parts.push(JsonValue::Array(items));
            }
            JsonValue::Array(parts)
        };
        context_key(&JsonValue::Array(vec![context, declared])).map_err(emit_error)
    })();
    match keyed {
        Err(error) if unstable(py, &error) => Ok(None),
        other => other,
    }
}
