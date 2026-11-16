//! Python's `find_macro_call_names`: top-level call names in encounter order, declarations excluded.

use crate::render::_helpers::calls::macro_calls::DECLARATION_CALLS;
use crate::render::_helpers::expansion::macro_expansion::raised;
use pyo3::prelude::{Bound, PyResult, Python};
use pyo3::types::PyTuple;
use sqlbuild_python_base::boundary::main::compiler_error::compiler_error;
use sqlbuild_python_base::records::main::construct::error_class::error_class;
use sqlbuild_python_base::records::types::ErrorId;
use sqlbuild_python_base::types::CompilerDetach;
use sqlbuild_render::macro_calls::main::macro_call_names::macro_call_names as scan_names;
use sqlbuild_sqltext::sql_scan::models::LexicalSyntax;

/// Python's `find_macro_call_names`: top-level call names in encounter order, declarations excluded.
pub fn top_level_macro_call_names<'py>(
    py: Python<'py>,
    sql: &str,
    syntax: Option<&LexicalSyntax>,
) -> PyResult<Bound<'py, PyTuple>> {
    let names = py
        .compiler_detach(|| Ok(scan_names(sql, false, syntax)))
        .map_err(compiler_error)?;
    match names {
        Ok(names) => PyTuple::new(
            py,
            names
                .into_iter()
                .filter(|name| !DECLARATION_CALLS.contains(&name.as_str()))
                .collect::<Vec<String>>(),
        ),
        Err(error) => Err(raised(
            &error_class(py, ErrorId::CompileInput)?,
            error.message(),
        )),
    }
}
