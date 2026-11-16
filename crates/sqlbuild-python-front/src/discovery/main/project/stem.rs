//! `str(Path(...).stem)` of the path attribute `name`.

use pyo3::prelude::{Bound, PyAny, PyResult};
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::main::fields::path_stem::path_stem;

/// `str(Path(...).stem)` of the path attribute `name`.
pub fn stem(owner: &Bound<'_, PyAny>, name: &str) -> PyResult<String> {
    path_stem(&native_field(owner, name)?)
}
