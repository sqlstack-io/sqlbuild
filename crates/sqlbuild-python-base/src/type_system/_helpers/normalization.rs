//! Native type normalization.

use pyo3::exceptions::PyValueError;
use pyo3::prelude::{Bound, PyErr, PyModule, PyModuleMethods, PyResult};
use pyo3::{pyfunction, wrap_pyfunction};
use sqlbuild_analysis::type_system::errors::TypeNormalizationError;
use sqlbuild_analysis::type_system::main::normalize_type::normalize_type;
use sqlbuild_analysis::type_system::models::{NormalizedType, TypeNormalization};

use crate::boundary::main::compiler_guard::compiler_guard;

/// `(name, family, precision, scale, length)` of one normalized type.
struct NormalizedRow {
    name: String,
    family: &'static str,
    precision: Option<String>,
    scale: Option<String>,
    length: Option<String>,
}

impl<'py> pyo3::IntoPyObject<'py> for NormalizedRow {
    type Target = pyo3::types::PyTuple;
    type Output = Bound<'py, pyo3::types::PyTuple>;
    type Error = PyErr;

    fn into_pyobject(self, py: pyo3::Python<'py>) -> PyResult<Self::Output> {
        use pyo3::IntoPyObjectExt;
        pyo3::types::PyTuple::new(
            py,
            [
                self.name.into_bound_py_any(py)?,
                self.family.into_bound_py_any(py)?,
                self.precision.into_bound_py_any(py)?,
                self.scale.into_bound_py_any(py)?,
                self.length.into_bound_py_any(py)?,
            ],
        )
    }
}

/// Normalize one type: `(normalized row, parse error)`, or Python's exception.
#[pyfunction]
#[pyo3(name = "normalize_type")]
fn normalize_type_binding(
    type_sql: &str,
    dialect: &str,
) -> PyResult<(NormalizedRow, Option<String>)> {
    let normalization: Result<TypeNormalization, TypeNormalizationError> =
        compiler_guard(|| Ok(normalize_type(type_sql, dialect)))?;
    match normalization {
        Ok(TypeNormalization {
            normalized,
            parse_error,
        }) => Ok((normalized_row(normalized), parse_error)),
        Err(error) => Err(normalization_error(&error)),
    }
}

/// Python's exception for a type that cannot be normalized.
pub(crate) fn normalization_error(error: &TypeNormalizationError) -> PyErr {
    PyValueError::new_err(error.message())
}

fn normalized_row(normalized: NormalizedType) -> NormalizedRow {
    NormalizedRow {
        name: normalized.normalized_name,
        family: normalized.family.as_str(),
        precision: normalized.precision.map(|value| value.as_str().to_owned()),
        scale: normalized.scale.map(|value| value.as_str().to_owned()),
        length: normalized.length.map(|value| value.as_str().to_owned()),
    }
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(normalize_type_binding, module)?)?;
    Ok(())
}
