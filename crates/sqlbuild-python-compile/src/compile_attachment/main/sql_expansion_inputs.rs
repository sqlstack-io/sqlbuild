//! Assemble SQL expansion inputs; a static scope lends only its index to private macros.

use crate::compile_attachment::_helpers::sql_expansion_context;
use crate::compile_attachment::models::ExpansionSources;
use pyo3::prelude::PyResult;
use sqlbuild_python_front::render::models::SqlExpansionInputs;

/// Assemble SQL expansion inputs; a static scope lends only its index to private macros.
#[inline]
pub fn sql_expansion_inputs(sources: ExpansionSources<'_>) -> PyResult<SqlExpansionInputs> {
    sql_expansion_context::sql_expansion_inputs(sources)
}
