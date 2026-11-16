//! `sql` expanded with lint's expansion inputs, as `file_path`'s declarations see them.

use crate::render::_helpers::declarations::authored_sql::AuthoredSources;
use crate::render::_helpers::declarations::authored_sql::authored_expansion;
use crate::render::_helpers::declarations::authored_sql::input_declarations;
use crate::render::models::AuthoredExpansion;
use crate::render::models::SqlExpansionInputs;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::PyString;

/// `sql` expanded with lint's expansion inputs, as `file_path`'s declarations see them.
pub fn expand_with_inputs(
    inputs: &SqlExpansionInputs,
    sql: &str,
    file_path: &Bound<'_, PyAny>,
) -> PyResult<AuthoredExpansion> {
    let py: Python<'_> = file_path.py();
    let resolver: Bound<'_, PyAny> = inputs.declaration_resolver.bind(py).clone();
    authored_expansion(AuthoredSources {
        sql: &PyString::new(py, sql).into_any(),
        file_path,
        effective_vars: inputs.effective_vars.bind(py),
        context_values: &py.None().into_bound(py),
        declarations: &input_declarations(inputs, file_path)?,
        value_renderer: inputs.value_renderer.bind(py),
        collection_rendering: inputs.collection_rendering.bind(py),
        loaded_macros: inputs.loaded_macros.bind(py),
        macro_context: inputs.macro_context.bind(py),
        declaration_resolver: (!resolver.is_none()).then_some(resolver),
    })
}
