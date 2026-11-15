//! Computed attributes of the natively hosted SQL value and fingerprint records.

use pyo3::IntoPyObjectExt;
use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods, PyTuple};
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::main::fields::mapping_values::mapping_values;

/// `SqlLogicalType.display_name`: the kind, with its element type in angle brackets.
pub(crate) fn logical_type_display_name(instance: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
    let kind: String = native_field(&native_field(instance, "kind")?, "value")?.extract()?;
    let element: Bound<'_, PyAny> = native_field(instance, "element_type")?;
    if element.is_none() {
        return kind.into_py_any(instance.py());
    }
    let inner: String = native_field(&element, "display_name")?.extract()?;
    format!("{kind}<{inner}>").into_py_any(instance.py())
}

/// `SqlValue.kind`: the outer logical kind.
pub(crate) fn sql_value_kind(instance: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
    Ok(native_field(&native_field(instance, "logical_type")?, "kind")?.unbind())
}

/// `FingerprintSet`: index the fingerprints by `(node_type, node_name)` when not given.
pub(crate) fn finish_fingerprint_set(
    py: Python<'_>,
    mut values: Vec<Py<PyAny>>,
) -> PyResult<Vec<Py<PyAny>>> {
    if !values[2].bind(py).is_none() {
        return Ok(values);
    }
    let by_identity: Bound<'_, PyDict> = PyDict::new(py);
    for fingerprint in mapping_values(values[1].bind(py))?.try_iter()? {
        let fingerprint: Bound<'_, PyAny> = fingerprint?;
        let identity: Bound<'_, PyTuple> = PyTuple::new(
            py,
            [
                native_field(&fingerprint, "node_type")?,
                native_field(&fingerprint, "node_name")?,
            ],
        )?;
        by_identity.set_item(identity, fingerprint)?;
    }
    values[2] = by_identity.into_any().unbind();
    Ok(values)
}
