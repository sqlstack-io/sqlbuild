//! Macro call-site scanning and splicing for the Python macro bridge, with the GIL released.

use pyo3::exceptions::PyValueError;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult, Python};
use pyo3::types::PyTuple;
use pyo3::{pyfunction, wrap_pyfunction};
use sqlbuild_render::macro_calls::main::macro_call_names::macro_call_names as scan_names;
use sqlbuild_render::macro_calls::main::scan_macro_call_sites::scan_macro_call_sites as scan_sites;
use sqlbuild_render::macro_calls::main::splice_macro_calls::splice_macro_calls as splice_calls;
use sqlbuild_render::macro_calls::models::{MacroCallScan, MacroScanFailure};

use crate::bindings::_helpers::boundary::panics::compiler_error;
use crate::bindings::_helpers::records::construct::error_class;
use crate::bindings::_helpers::records::ids::ErrorId;
use crate::bindings::_helpers::render::macro_call_memo::MacroCallMemo;
use crate::bindings::_helpers::render::macro_call_rows::{
    SiteRow, SpanTuple, site_row, span_row, span_tuple,
};
use crate::bindings::_helpers::render::macro_expansion::raised;
use crate::bindings::types::CompilerDetach;
use sqlbuild_sqltext::sql_scan::models::LexicalSyntax;

const DECLARATION_CALLS: [&str; 2] = ["enum", "const"];

/// `(call start or None, call name or None, message)` of the error Python's scan raises.
type FailureRow = (Option<usize>, Option<String>, &'static str);

/// Every complete top-level macro call site of `sql`, and where Python's scan raises, if it does.
#[pyfunction]
pub(crate) fn scan_macro_call_sites(
    py: Python<'_>,
    sql: &str,
) -> PyResult<(Vec<SiteRow>, Option<FailureRow>)> {
    let MacroCallScan { sites, failure } = py
        .compiler_detach(|| Ok(scan_sites(sql, None)))
        .map_err(compiler_error)?;
    Ok((
        sites.into_iter().map(site_row).collect(),
        failure.map(
            |MacroScanFailure {
                 call_start,
                 call_name,
                 error,
             }| (call_start, call_name, error.message()),
        ),
    ))
}

/// Python's `find_macro_call_names`: top-level call names in encounter order, declarations excluded.
pub(crate) fn top_level_macro_call_names<'py>(
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

/// Unique authored macro call names in encounter order, declaration calls excluded.
#[pyfunction]
fn find_macro_call_names<'py>(py: Python<'py>, sql: &str) -> PyResult<Bound<'py, PyTuple>> {
    top_level_macro_call_names(py, sql, None)
}

/// Splice rendered macro outputs over their call sites, returning the SQL and substitution spans.
#[pyfunction]
pub(crate) fn splice_macro_calls(
    py: Python<'_>,
    sql: &str,
    sites: Vec<(usize, usize)>,
    outputs: Vec<String>,
) -> PyResult<(String, Vec<SpanTuple>)> {
    let spliced = py
        .compiler_detach(|| Ok(splice_calls(sql, &sites, &outputs)))
        .map_err(compiler_error)?;
    let (rendered, spans) =
        spliced.ok_or_else(|| PyValueError::new_err("macro call sites do not fit the SQL"))?;
    Ok((
        rendered,
        spans
            .into_iter()
            .map(|span| span_tuple(&span_row(span)))
            .collect(),
    ))
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(scan_macro_call_sites, module)?)?;
    module.add_function(wrap_pyfunction!(find_macro_call_names, module)?)?;
    module.add_function(wrap_pyfunction!(splice_macro_calls, module)?)?;
    module.add_class::<MacroCallMemo>()?;
    Ok(())
}
