//! Native type normalization.

use pyo3::exceptions::PyValueError;
use pyo3::prelude::{Bound, PyErr, PyModule, PyModuleMethods, PyResult};
use pyo3::{pyfunction, wrap_pyfunction};
use sqlbuild_analysis::type_system::errors::TypeNormalizationError;
use sqlbuild_analysis::type_system::main::normalize_type::normalize_type;
use sqlbuild_analysis::type_system::models::{NormalizedType, TypeNormalization};

use crate::bindings::_helpers::boundary::panics::compiler_guard;

positional_record! {
    into;
    /// `(name, family, precision, scale, length)` of one normalized type.
    struct NormalizedRow {
        name: String,
        family: &'static str,
        precision: Option<String>,
        scale: Option<String>,
        length: Option<String>,
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
