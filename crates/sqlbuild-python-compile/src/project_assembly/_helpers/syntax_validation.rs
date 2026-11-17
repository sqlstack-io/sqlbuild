//! Reject model and hook SQL the parser cannot read, naming the model, hook and file.

use crate::project_assembly::main::sql_entry_label::sql_entry_label;
use crate::project_assembly::main::syntax_error_message::syntax_error_message;
use sqlbuild_python_base::records::main::construct::new_error::new_error;
use std::collections::HashMap;

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyErr, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::{PyList, PyString, PyTuple};
use sqlbuild_analysis::assembly::project::main::sql_analysis_opt_out_help::sql_analysis_opt_out_help;
use sqlbuild_analysis::assembly::project::models::{SyntaxCheck, SyntaxMode};

use sqlbuild_python_base::records::main::construct::record_class::record_class;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::ErrorId;
use sqlbuild_python_base::records::types::RecordId;

pub(crate) const GENERIC_DIALECT: &str = "generic";

/// What every statement of one model is parsed with.
pub(crate) struct SyntaxContext<'a, 'py> {
    pub(crate) py: Python<'py>,
    pub(crate) model_name: &'a str,
    pub(crate) placeholders: &'a Option<HashMap<String, String>>,
    pub(crate) dialect: &'a str,
    pub(crate) sql_hook_entry: Bound<'py, PyAny>,
    pub(crate) python_hook_entry: Bound<'py, PyAny>,
}

/// One hook value to validate: the hook key, its file and the label errors name it by.
pub(crate) struct HookSite<'a, 'py> {
    pub(crate) hook_name: &'a str,
    pub(crate) file_path: Bound<'py, PyAny>,
    pub(crate) label: Option<String>,
}

/// `CompileInputError(message, help=sql_analysis_opt_out_help(model_owned=True))`.
pub(crate) fn syntax_error(py: Python<'_>, message: String) -> PyErr {
    new_error(
        py,
        ErrorId::CompileInput,
        &message,
        (None, Some(&sql_analysis_opt_out_help(true))),
    )
}

/// The parser's message for `sql`, or `None` where it parses.
fn parse_error(context: &SyntaxContext<'_, '_>, sql: &str) -> PyResult<Option<String>> {
    let check: SyntaxCheck = SyntaxCheck {
        sql: sql.to_owned(),
        placeholders: context
            .placeholders
            .clone()
            .unwrap_or_default()
            .into_iter()
            .collect(),
    };
    syntax_error_message(context.py, &check, context.dialect, SyntaxMode::Validate)
}

/// `validate_hook_sql_syntax`: each SQL string inside a hook value, depth first.
pub(crate) fn check_hook_value(
    context: &SyntaxContext<'_, '_>,
    value: &Bound<'_, PyAny>,
    site: HookSite<'_, '_>,
) -> PyResult<()> {
    if value.is_instance_of::<PyString>() {
        let label: String = site.label.unwrap_or_else(|| site.hook_name.to_owned());
        if let Some(message) = parse_error(context, &value.extract::<String>()?)? {
            return Err(syntax_error(
                context.py,
                format!(
                    "Polyglot could not parse model '{}' {label} ({}): {message}",
                    context.model_name,
                    site.file_path.str()?
                ),
            ));
        }
        return Ok(());
    }
    if value.is_instance(&context.sql_hook_entry)? {
        let label: String = match site.label {
            Some(label) => label,
            None => sql_entry_label(site.hook_name, value)?,
        };
        let relative_path: Bound<'_, PyAny> = native_field(value, "relative_path")?;
        let file_path = if relative_path.is_truthy()? {
            relative_path
        } else {
            site.file_path
        };
        let statement: Bound<'_, PyAny> = native_field(value, "statement")?;
        return check_hook_value(
            context,
            &statement,
            HookSite {
                hook_name: site.hook_name,
                file_path,
                label: Some(label),
            },
        );
    }
    if value.is_instance(&context.python_hook_entry)? {
        return Ok(());
    }
    if value.is_instance_of::<PyList>() || value.is_instance_of::<PyTuple>() {
        for (index, item) in value.try_iter()?.enumerate() {
            let item: Bound<'_, PyAny> = item?;
            let prefix: String = format!("{}[{index}]", site.hook_name);
            let label: Option<String> = if item.is_instance(&context.sql_hook_entry)? {
                Some(sql_entry_label(&prefix, &item)?)
            } else if item.is_instance_of::<PyString>() {
                Some(format!("{prefix} inline_sql(\"...\")"))
            } else {
                None
            };
            check_hook_value(
                context,
                &item,
                HookSite {
                    hook_name: site.hook_name,
                    file_path: site.file_path.clone(),
                    label,
                },
            )?;
        }
    }
    Ok(())
}

/// `hook_sql_statements`: the hook SQL strings `validate_hook_sql_syntax` checks, in its order.
pub(crate) fn hook_statements(
    value: &Bound<'_, PyAny>,
    sql_hook_entry: &Bound<'_, PyAny>,
) -> PyResult<Vec<String>> {
    if value.is_instance_of::<PyString>() {
        return Ok(vec![value.extract()?]);
    }
    if value.is_instance(sql_hook_entry)? {
        return hook_statements(&native_field(value, "statement")?, sql_hook_entry);
    }
    let mut statements: Vec<String> = Vec::new();
    if value.is_instance_of::<PyList>() || value.is_instance_of::<PyTuple>() {
        for item in value.try_iter()? {
            statements.extend(hook_statements(&item?, sql_hook_entry)?);
        }
    }
    Ok(statements)
}

/// The hook SQL strings a hook value holds, in validation order.
#[pyfunction]
pub(crate) fn hook_sql_statements(value: Bound<'_, PyAny>) -> PyResult<Vec<String>> {
    let sql_hook_entry: Bound<'_, PyAny> = record_class(value.py(), RecordId::SqlHookEntry)?;
    hook_statements(&value, &sql_hook_entry)
}

/// Help for SQL the parser rejects; only model-owned SQL offers the MODEL header entry.
#[pyfunction]
#[pyo3(name = "sql_analysis_opt_out_help", signature = (*, model_owned = true))]
pub(crate) fn opt_out_help(model_owned: bool) -> String {
    sql_analysis_opt_out_help(model_owned)
}
