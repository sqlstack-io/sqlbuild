//! The relation a model or seed destination names; a destination without a schema has an empty one.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_front::native_host::main::optional_text::optional_text;
use sqlbuild_state::consumption::models::RelationKey;

/// The relation a model or seed destination names; a destination without a schema has an empty one.
pub fn relation(destination: &Bound<'_, PyAny>) -> PyResult<RelationKey> {
    Ok(RelationKey {
        database: optional_text(&native_field(destination, "database")?)?,
        schema: optional_text(&native_field(destination, "schema")?)?.unwrap_or_default(),
        name: native_field(destination, "name")?.extract()?,
    })
}
