//! Each reported diagnostic of `rows` through `(authored, query, cleaned)` and the expansion.

use crate::analysis::_helpers::binding_locations::located_row;
use crate::analysis::models::LocatedRow;
use crate::analysis::types::ExpansionRow;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::PyResult;
use pyo3::pyfunction;
use sqlbuild_analysis::binding_locations::main::locate_binding_diagnostics::locate_binding_diagnostics;
use sqlbuild_analysis::binding_locations::models::{BindingSource, LocatedDiagnostic};
use sqlbuild_analysis::semantic_validation::models::{DiagnosticRow, Expansion};
use sqlbuild_python_project::analysis::models::Positional;

/// Each reported diagnostic of `rows` through `(authored, query, cleaned)` and the expansion.
#[pyfunction]
pub fn locate_model_binding_diagnostics(
    rows: Vec<Positional<DiagnosticRow>>,
    texts: (String, String, String),
    expansion: ExpansionRow,
) -> PyResult<Vec<LocatedRow>> {
    let (authored, query, cleaned) = texts;
    let rows: Vec<DiagnosticRow> = Positional::unwrap_all(rows);
    let expansion: Option<(String, Vec<Vec<Expansion>>)> = expansion.map(|(expanded, passes)| {
        (
            expanded,
            passes.into_iter().map(Positional::unwrap_all).collect(),
        )
    });
    let source: BindingSource<'_> = BindingSource {
        authored: &authored,
        query: &query,
        cleaned: &cleaned,
        expansion: expansion
            .as_ref()
            .map(|(expanded, passes)| (expanded.as_str(), passes.as_slice())),
    };
    let located: Vec<LocatedDiagnostic> =
        locate_binding_diagnostics(&rows, &source).map_err(PyValueError::new_err)?;
    Ok(located.into_iter().map(located_row).collect())
}
