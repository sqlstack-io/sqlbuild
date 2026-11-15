//! Computed attributes of the natively hosted discovery and pipeline records.

use pyo3::IntoPyObjectExt;
use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyResult};
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// SQL scalar type labels by logical value kind.
const SCALAR_TYPE_LABELS: [(&str, &str); 6] = [
    ("string", "VARCHAR"),
    ("integer", "INTEGER"),
    ("boolean", "BOOLEAN"),
    ("float", "FLOAT"),
    ("decimal", "DECIMAL"),
    ("null", "NULL"),
];

/// `ConstantDeclaration.logical_type`: the normalized logical type.
pub(crate) fn constant_logical_type(declaration: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
    Ok(native_field(&native_field(declaration, "value")?, "logical_type")?.unbind())
}

/// `ConstantDeclaration.scalar_type`: the SQL scalar type label, or the upper-cased logical type.
pub(crate) fn constant_scalar_type(declaration: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
    let value: Bound<'_, PyAny> = native_field(declaration, "value")?;
    let kind: String = native_field(&value, "kind")?.str()?.extract()?;
    if let Some((_, label)) = SCALAR_TYPE_LABELS.iter().find(|(name, _)| *name == kind) {
        return label.into_py_any(declaration.py());
    }
    let display: String =
        native_field(&native_field(&value, "logical_type")?, "display_name")?.extract()?;
    display.to_uppercase().into_py_any(declaration.py())
}
