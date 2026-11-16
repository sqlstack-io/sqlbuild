//! The `CompiledRelationLocation` of a destination.

use crate::compiled_views::_helpers::compiled_models::compiled_model_view::DATABASE_KEY;
use crate::compiled_views::_helpers::compiled_models::compiled_model_view::LOGICAL_DATABASE_ATTRIBUTE;
use crate::compiled_views::_helpers::compiled_models::compiled_model_view::LOGICAL_SCHEMA_ATTRIBUTE;
use crate::compiled_views::_helpers::compiled_models::compiled_model_view::SCHEMA_KEY;
use crate::compiled_views::models::CompiledViewClasses;
use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyResult};
use pyo3::types::{PyDict, PyDictMethods};
use sqlbuild_project::models::DestinationFact;

/// The `CompiledRelationLocation` of a destination.
pub fn relation_location(
    classes: &CompiledViewClasses<'_>,
    destination: &DestinationFact,
) -> PyResult<Py<PyAny>> {
    let fields: Bound<'_, PyDict> = PyDict::new(classes.relation_location.py());
    fields.set_item(DATABASE_KEY, &destination.database)?;
    fields.set_item(SCHEMA_KEY, &destination.schema)?;
    fields.set_item("name", &destination.name)?;
    fields.set_item("qualified_name", &destination.qualified_name)?;
    fields.set_item(LOGICAL_SCHEMA_ATTRIBUTE, &destination.logical_schema)?;
    fields.set_item(LOGICAL_DATABASE_ATTRIBUTE, &destination.logical_database)?;
    Ok(classes.relation_location.call((), Some(&fields))?.unbind())
}
