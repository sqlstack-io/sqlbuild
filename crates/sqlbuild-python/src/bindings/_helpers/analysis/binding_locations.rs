//! A hand-built model's binding diagnostics at their authored locations.

use pyo3::exceptions::PyValueError;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::{pyfunction, wrap_pyfunction};
use sqlbuild_analysis::binding_locations::main::locate_binding_diagnostics::locate_binding_diagnostics;
use sqlbuild_analysis::binding_locations::models::{
    AuthoredSpan, BindingSource, LocatedDiagnostic,
};
use sqlbuild_analysis::semantic_validation::models::{DiagnosticRow, Expansion};
use sqlbuild_python_base::positional_record;

use crate::bindings::models::Positional;

positional_record! {
    into;
    /// One located diagnostic as Python reads it, its span `(line, column, end line, end column)`.
    pub(crate) struct LocatedRow {
        pub(crate) code: String,
        pub(crate) message: String,
        pub(crate) severity: String,
        pub(crate) line: Option<usize>,
        pub(crate) column: Option<usize>,
        pub(crate) span: Option<Positional<AuthoredSpan>>,
    }
}

/// The expanded SQL and the expansion passes Python sends for a located model.
pub(crate) type ExpansionRow = Option<(String, Vec<Vec<Positional<Expansion>>>)>;

/// Each reported diagnostic of `rows` through `(authored, query, cleaned)` and the expansion.
#[pyfunction]
pub(crate) fn locate_model_binding_diagnostics(
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

fn located_row(item: LocatedDiagnostic) -> LocatedRow {
    LocatedRow {
        code: item.code,
        message: item.message,
        severity: item.severity,
        line: item.line,
        column: item.column,
        span: item.location.map(Positional),
    }
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(locate_model_binding_diagnostics, module)?)?;
    Ok(())
}
