//! Lint and format SQL with the native rules engine.

use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult, Python};
use pyo3::{FromPyObject, pyfunction, wrap_pyfunction};
use sqlbuild_rules::lint_text::models::InterpolationSite;
use sqlbuild_rules::lint_text::types::PreparedSql;

use sqlbuild_python_base::boundary::main::value_error::value_error;
use sqlbuild_python_base::types::CompilerDetach;
use sqlbuild_python_project::analysis::models::Positional;
use sqlbuild_python_project::compiled_views::main::compiled_models::register_relation_keys::register_relation_keys;

#[pyfunction]
fn lint_sql_json(py: Python<'_>, request_json: &str) -> PyResult<String> {
    py.compiler_detach(|| sqlclean::sql_lint::main::engine::lint_json(request_json))
        .map_err(value_error)
}

/// One SQL lint preparation request, read from a Python mapping.
#[derive(FromPyObject)]
#[pyo3(from_item_all)]
struct LintPreparationRequest {
    expanded: String,
    before_expansion: String,
    prior_sites: Vec<usize>,
    dialect: String,
}

/// [`PreparedSql`] as Python reads it, each site a positional tuple.
type PreparedLintSql = (String, Vec<Positional<InterpolationSite>>, Vec<String>);

#[pyfunction]
fn prepare_lint_sql(
    py: Python<'_>,
    request: LintPreparationRequest,
) -> PyResult<Option<PreparedLintSql>> {
    let prepared: Option<PreparedSql> = py
        .compiler_detach(|| {
            sqlbuild_rules::lint_text::main::preparation::prepare(
                &request.expanded,
                &request.before_expansion,
                &request.prior_sites,
                &request.dialect,
            )
        })
        .map_err(value_error)?;
    Ok(prepared.map(|(text, sites, referenced)| {
        (
            text,
            sites.into_iter().map(Positional).collect(),
            referenced,
        )
    }))
}

#[pyfunction]
fn format_sql_json(py: Python<'_>, request_json: &str) -> PyResult<String> {
    py.compiler_detach(|| sqlclean::sql_lint::main::formatter::format_json(request_json))
        .map_err(value_error)
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(lint_sql_json, module)?)?;
    module.add_function(wrap_pyfunction!(prepare_lint_sql, module)?)?;
    module.add_function(wrap_pyfunction!(format_sql_json, module)?)?;
    register_relation_keys(module)
}
