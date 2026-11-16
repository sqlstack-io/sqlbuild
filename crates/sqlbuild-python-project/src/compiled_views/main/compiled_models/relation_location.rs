//! The `CompiledRelationLocation` of a destination.

use crate::compiled_views::_helpers::compiled_models::compiled_model_view;
use crate::compiled_views::models::CompiledViewClasses;
use pyo3::prelude::Py;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use sqlbuild_project::models::DestinationFact;

/// The `CompiledRelationLocation` of a destination.
#[inline]
pub fn relation_location(
    classes: &CompiledViewClasses<'_>,
    destination: &DestinationFact,
) -> PyResult<Py<PyAny>> {
    compiled_model_view::relation_location(classes, destination)
}
