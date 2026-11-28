//! The source texts a changed Python node's identity diffs compare.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyModule, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods, PyList, PyListMethods};
use sqlbuild_python_base::boundary::main::text_item::text_item;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

const DEPENDENCY_HEADER_SEPARATOR: &str = " :: ";
const DEPENDENCY_HEADER_FALLBACK: &str = "dependency";
const HEADER_FIELDS: [&str; 3] = ["source_path", "module", "qualname"];

/// The JSON object a recorded identity holds, or `None` when it is missing or not an object.
fn json_object<'py>(raw_json: &Bound<'py, PyAny>) -> PyResult<Option<Bound<'py, PyDict>>> {
    let py: Python<'py> = raw_json.py();
    if raw_json.is_none() {
        return Ok(None);
    }
    let json: Bound<'py, PyModule> = py.import("json")?;
    let payload: Bound<'py, PyAny> = match json.call_method1("loads", (raw_json,)) {
        Ok(payload) => payload,
        Err(error) if error.is_instance(py, &native_field(&json, "JSONDecodeError")?) => {
            return Ok(None);
        }
        Err(error) => return Err(error),
    };
    let Ok(payload) = payload.cast_into::<PyDict>() else {
        return Ok(None);
    };
    Ok(Some(payload))
}

/// The recorded definition's `source_text`.
pub(crate) fn definition_source_text(raw_json: &Bound<'_, PyAny>) -> PyResult<Option<String>> {
    match json_object(raw_json)? {
        Some(payload) => text_item(&payload, "source_text"),
        None => Ok(None),
    }
}

/// Each dependency's source under a `# path :: module :: qualname` header, sorted by that header.
pub(crate) fn dependency_source_text(raw_json: &Bound<'_, PyAny>) -> PyResult<Option<String>> {
    let Some(payload) = json_object(raw_json)? else {
        return Ok(None);
    };
    let Some(dependencies) = payload.get_item("dependencies")? else {
        return Ok(None);
    };
    let Ok(dependencies) = dependencies.cast::<PyList>() else {
        return Ok(None);
    };
    let mut keyed: Vec<([String; 3], Option<Bound<'_, PyDict>>)> = Vec::new();
    for dependency in dependencies.iter() {
        let Ok(dependency) = dependency.cast_into::<PyDict>() else {
            keyed.push((Default::default(), None));
            continue;
        };
        let mut key: [String; 3] = Default::default();
        for (slot, field) in key.iter_mut().zip(HEADER_FIELDS) {
            *slot = text_item(&dependency, field)?.unwrap_or_default();
        }
        keyed.push((key, Some(dependency)));
    }
    keyed.sort_by(|left, right| left.0.cmp(&right.0));
    let mut blocks: Vec<String> = Vec::new();
    for (key, dependency) in keyed {
        let Some(dependency) = dependency else {
            continue;
        };
        let Some(source_text) = text_item(&dependency, "source_text")? else {
            continue;
        };
        let parts: Vec<&str> = key
            .iter()
            .map(String::as_str)
            .filter(|part| !part.is_empty())
            .collect();
        let header: String = if parts.is_empty() {
            DEPENDENCY_HEADER_FALLBACK.to_owned()
        } else {
            parts.join(DEPENDENCY_HEADER_SEPARATOR)
        };
        blocks.push(format!("# {header}\n{source_text}"));
    }
    Ok(Some(blocks.join("\n\n")))
}
