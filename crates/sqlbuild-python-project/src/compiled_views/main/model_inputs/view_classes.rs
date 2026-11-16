//! The classes the owner's views project attributes with.

use crate::compiled_project::models::NativeCompiledProject;
use crate::compiled_views::_helpers::model_inputs::model_input_view;
use crate::compiled_views::models::InputViewClasses;
use pyo3::prelude::Py;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;

/// The classes the owner's views project attributes with.
#[inline]
pub fn view_classes<'py>(
    owner: &Py<NativeCompiledProject>,
    py: Python<'py>,
) -> PyResult<InputViewClasses<'py>> {
    model_input_view::view_classes(owner, py)
}
