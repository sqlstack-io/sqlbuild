//! Build one canonical index and validate it before SQL expansion.

use crate::compile_attachment::_helpers::declaration_scope;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// Build one canonical index and validate it before SQL expansion.
#[inline]
pub fn build_declaration_scope<'py>(
    discovered_inputs: &Bound<'py, PyAny>,
    loaded_macros: &Bound<'py, PyAny>,
    sql_lexical_syntax: &Bound<'py, PyAny>,
    compile_cache_dir: Option<Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    declaration_scope::build_declaration_scope(
        discovered_inputs,
        loaded_macros,
        sql_lexical_syntax,
        compile_cache_dir,
    )
}
