//! SQL macro files parsed and checked when the project's macros load.

use pyo3::prelude::{Bound, PyErr, PyModule, PyModuleMethods, PyResult, Python};
use pyo3::{pyfunction, wrap_pyfunction};
use sqlbuild_analysis::sql_macros::constants::CYCLE_CODE;
use sqlbuild_analysis::sql_macros::errors::SqlMacroError;
use sqlbuild_analysis::sql_macros::main::parse_sql_macro::parse_sql_macro;
use sqlbuild_analysis::sql_macros::main::sql_macro_cycle::sql_macro_cycle;
use sqlbuild_analysis::sql_macros::models::{SqlMacroDefinition, SqlMacroLocation};

use sqlbuild_python_base::records::main::construct::new_helped_error::new_helped_error;
use sqlbuild_python_base::records::types::ErrorId;

/// `(name, relative path, contents)` of one SQL macro file.
type SourceRow = (String, String, String);
/// `(name, parameter names, called SQL macros)` of one parsed SQL macro.
type DefinitionRow = (String, Vec<String>, Vec<String>);

fn raised(py: Python<'_>, error: &SqlMacroError) -> PyErr {
    new_helped_error(
        py,
        ErrorId::CompileInput,
        (error.located_message(None), error.code),
        error.help.as_deref(),
    )
}

/// The `(name, parameter names, called SQL macros)` row of one parsed SQL macro.
fn definition_row(definition: SqlMacroDefinition) -> DefinitionRow {
    let parameters: Vec<String> = definition
        .parameters
        .into_iter()
        .map(|parameter| parameter.name)
        .collect();
    (definition.name, parameters, definition.calls)
}

/// Each SQL macro's parameters and calls, or the first file that is not a valid SQL macro.
#[pyfunction]
fn inventory_sql_macros(py: Python<'_>, sources: Vec<SourceRow>) -> PyResult<Vec<DefinitionRow>> {
    let mut definitions: Vec<SqlMacroDefinition> = Vec::with_capacity(sources.len());
    for (name, path, contents) in &sources {
        definitions
            .push(parse_sql_macro(name, path, contents).map_err(|error| raised(py, &error))?);
    }
    let calls: Vec<(String, Vec<String>)> = definitions
        .iter()
        .map(|definition| (definition.name.clone(), definition.calls.clone()))
        .collect();
    if let Some(cycle) = sql_macro_cycle(&calls) {
        let closing: &SqlMacroDefinition = definitions
            .iter()
            .find(|definition| definition.name == cycle[cycle.len() - 2])
            .unwrap_or(&definitions[0]);
        let shown: Vec<String> = cycle.iter().map(|name| format!("{name}()")).collect();
        let error = SqlMacroError::new_public(
            CYCLE_CODE,
            format!(
                "SQL macros call each other in a cycle: {}",
                shown.join(" -> ")
            ),
        )
        .at(SqlMacroLocation {
            path: closing.path.clone(),
            line: closing.body_line,
            column: closing.body_column,
        })
        .with_help("SQL macros may call other SQL macros, but not back into themselves");
        return Err(raised(py, &error));
    }
    Ok(definitions.into_iter().map(definition_row).collect())
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(inventory_sql_macros, module)?)
}
