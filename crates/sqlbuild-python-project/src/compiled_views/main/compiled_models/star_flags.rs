//! `(has star, star resolved)` of the model's analysed query.

use crate::compiled_views::main::compiled_models::native_model::native_model;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// `(has star, star resolved)` of the model's analysed query.
pub fn star_flags(model: &Bound<'_, PyAny>) -> PyResult<(bool, bool)> {
    if let Some(view) = native_model(model)? {
        return Ok((
            view.fast_lineage_has_star(model.py())?,
            view.fast_lineage_star_resolved(model.py())?,
        ));
    }
    Ok((
        native_field(model, "fast_lineage_has_star")?.is_truthy()?,
        native_field(model, "fast_lineage_star_resolved")?.is_truthy()?,
    ))
}
