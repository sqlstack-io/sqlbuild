//! Macro call-site scanning and splicing for the Python macro bridge, with the GIL released.

use crate::render::main::calls::span_row::span_row;
use crate::render::main::calls::span_tuple::span_tuple;
use crate::render::main::calls::top_level_macro_call_names::top_level_macro_call_names;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::{Bound, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::PyTuple;
use sqlbuild_render::macro_calls::main::scan_macro_call_sites::scan_macro_call_sites as scan_sites;
use sqlbuild_render::macro_calls::main::splice_macro_calls::splice_macro_calls as splice_calls;
use sqlbuild_render::macro_calls::models::{MacroCallScan, MacroScanFailure};

use crate::render::_helpers::calls::macro_call_rows::{SiteRow, site_row};
use crate::render::models::SpanTuple;
use sqlbuild_python_base::boundary::main::compiler_error::compiler_error;
use sqlbuild_python_base::types::CompilerDetach;

pub(crate) const DECLARATION_CALLS: [&str; 2] = ["enum", "const"];

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

/// Unique authored macro call names in encounter order, declaration calls excluded.
#[pyfunction]
pub(crate) fn find_macro_call_names<'py>(
    py: Python<'py>,
    sql: &str,
) -> PyResult<Bound<'py, PyTuple>> {
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
        .compiler_detach(|| Ok(splice_calls(sql, &sites, &outputs, None)))
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
