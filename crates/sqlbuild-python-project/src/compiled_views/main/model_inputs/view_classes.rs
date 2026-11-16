//! The classes the owner's views project attributes with.

use crate::compiled_project::models::NativeCompiledProject;
use crate::compiled_views::models::InputViewClasses;
use pyo3::prelude::{Py, PyAnyMethods, PyResult, Python};

/// The classes the owner's views project attributes with.
pub fn view_classes<'py>(
    owner: &Py<NativeCompiledProject>,
    py: Python<'py>,
) -> PyResult<InputViewClasses<'py>> {
    owner.bind(py).borrow().input_classes(py)?.extract()
}
