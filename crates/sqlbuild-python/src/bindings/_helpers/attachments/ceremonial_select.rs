//! Where a SQL test or scenario body omits its trailing ceremonial `SELECT 1`.

use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::{pyfunction, wrap_pyfunction};
use sqlbuild_attachments::ceremonial_select::main::omitted_select_offset::omitted_select_offset;
use sqlbuild_attachments::ceremonial_select::models::OmittedSelect;
use sqlbuild_sqltext::sql_scan::models::LexicalSyntax;

use crate::bindings::_helpers::sqltext::lexical_syntax::LexicalSyntaxInput;
use sqlbuild_python_base::boundary::main::compiler_guard::compiler_guard;

/// The code-point offset to insert `SELECT 1` at, or `None`.
#[pyfunction]
fn omitted_ceremonial_select(sql: &str, syntax: LexicalSyntaxInput) -> PyResult<Option<usize>> {
    let syntax: LexicalSyntax = syntax.into();
    compiler_guard(|| {
        Ok(match omitted_select_offset(sql, &syntax) {
            OmittedSelect::At(offset) => Some(offset),
            OmittedSelect::Absent => None,
        })
    })
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(omitted_ceremonial_select, module)?)?;
    Ok(())
}
