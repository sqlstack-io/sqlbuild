//! Validate the SQL of one model's hook value, raising the first parse failure.

use crate::project_assembly::_helpers::syntax_validation::GENERIC_DIALECT;
use crate::project_assembly::_helpers::syntax_validation::HookSite;
use crate::project_assembly::_helpers::syntax_validation::SyntaxContext;
use crate::project_assembly::_helpers::syntax_validation::check_hook_value;
use crate::project_assembly::types::ParseOptions;
use pyo3::prelude::{Bound, PyAny, PyErr, Python};
use pyo3::pyfunction;
use sqlbuild_python_base::records::main::construct::record_class::record_class;
use sqlbuild_python_base::records::types::RecordId;

/// Validate the SQL of one model's hook value, raising the first parse failure.
#[pyfunction]
#[pyo3(signature = (value, hook_name, model, parsing=(None, None)))]
pub fn validate_hook_sql_syntax(
    value: Bound<'_, PyAny>,
    hook_name: &str,
    model: (String, Bound<'_, PyAny>),
    parsing: ParseOptions,
) -> Result<(), PyErr> {
    let (model_name, file_path) = model;
    let (placeholders, dialect) = parsing;
    let py: Python<'_> = value.py();
    let dialect: String = dialect
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| GENERIC_DIALECT.to_owned());
    let context: SyntaxContext<'_, '_> = SyntaxContext {
        py,
        model_name: &model_name,
        placeholders: &placeholders,
        dialect: &dialect,
        sql_hook_entry: record_class(py, RecordId::SqlHookEntry)?,
        python_hook_entry: record_class(py, RecordId::PythonHookEntry)?,
    };
    check_hook_value(
        &context,
        &value,
        HookSite {
            hook_name,
            file_path,
            label: None,
        },
    )
}
