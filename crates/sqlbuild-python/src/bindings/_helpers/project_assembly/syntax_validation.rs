//! Reject model and hook SQL the parser cannot read, naming the model, hook and file.

use std::collections::HashMap;

use pyo3::prelude::{
    Bound, PyAny, PyAnyMethods, PyErr, PyModule, PyModuleMethods, PyResult, Python,
};
use pyo3::types::{PyDict, PyList, PyString, PyTuple};
use pyo3::{pyfunction, wrap_pyfunction};
use sqlbuild_analysis::assembly::project::main::sql_analysis_opt_out_help::sql_analysis_opt_out_help;
use sqlbuild_analysis::assembly::project::models::{SyntaxCheck, SyntaxMode};

use crate::bindings::_helpers::project_assembly::resources::syntax_error_message;
use crate::bindings::_helpers::records::construct::error_class;
use crate::bindings::_helpers::records::construct::record_class;
use crate::bindings::_helpers::records::fields::field as native_field;
use crate::bindings::_helpers::records::ids::{ErrorId, RecordId};

const GENERIC_DIALECT: &str = "generic";

/// `(placeholder defaults, dialect)` a statement parses with.
pub(crate) type ParseOptions = (Option<HashMap<String, String>>, Option<String>);

/// What every statement of one model is parsed with.
struct SyntaxContext<'a, 'py> {
    py: Python<'py>,
    model_name: &'a str,
    placeholders: &'a Option<HashMap<String, String>>,
    dialect: &'a str,
    sql_hook_entry: Bound<'py, PyAny>,
    python_hook_entry: Bound<'py, PyAny>,
}

/// One hook value to validate: the hook key, its file and the label errors name it by.
struct HookSite<'a, 'py> {
    hook_name: &'a str,
    file_path: Bound<'py, PyAny>,
    label: Option<String>,
}

/// `CompileInputError(message, help=sql_analysis_opt_out_help(model_owned=True))`.
fn syntax_error(py: Python<'_>, message: String) -> PyErr {
    let raised = || -> PyResult<PyErr> {
        let keywords: Bound<'_, PyDict> = PyDict::new(py);
        keywords.set_item("help", sql_analysis_opt_out_help(true))?;
        let error: Bound<'_, PyAny> =
            error_class(py, ErrorId::CompileInput)?.call((message,), Some(&keywords))?;
        Ok(PyErr::from_value(error))
    };
    raised().unwrap_or_else(|error| error)
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

/// How errors name a SQL hook entry: `{prefix} sql("name")`, or `{prefix} inline_sql("...")`.
pub(crate) fn sql_entry_label(prefix: &str, entry: &Bound<'_, PyAny>) -> PyResult<String> {
    let name: Bound<'_, PyAny> = native_field(entry, "name")?;
    Ok(if name.is_none() {
        format!("{prefix} inline_sql(\"...\")")
    } else {
        format!("{prefix} sql(\"{}\")", name.str()?)
    })
}

/// `validate_hook_sql_syntax`: each SQL string inside a hook value, depth first.
fn check_hook_value(
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

/// Validate the SQL of one model's hook value, raising the first parse failure.
#[pyfunction]
#[pyo3(signature = (value, hook_name, model, parsing=(None, None)))]
pub(crate) fn validate_hook_sql_syntax(
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

/// Validate that a model's query parses, raising its syntax error.
#[pyfunction]
#[pyo3(signature = (query_sql, model, parsing=(None, None)))]
pub(crate) fn validate_sql_syntax(
    py: Python<'_>,
    query_sql: &str,
    model: (String, Bound<'_, PyAny>),
    parsing: ParseOptions,
) -> Result<(), PyErr> {
    let (model_name, file_path) = model;
    let (placeholders, dialect) = parsing;
    let check: SyntaxCheck = SyntaxCheck {
        sql: query_sql.to_owned(),
        placeholders: placeholders.unwrap_or_default().into_iter().collect(),
    };
    let dialect: String = dialect
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| GENERIC_DIALECT.to_owned());
    match syntax_error_message(py, &check, &dialect, SyntaxMode::Validate)? {
        Some(message) => Err(syntax_error(
            py,
            format!(
                "SQL syntax error in model '{model_name}' ({}): {message}",
                file_path.str()?
            ),
        )),
        None => Ok(()),
    }
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
fn opt_out_help(model_owned: bool) -> String {
    sql_analysis_opt_out_help(model_owned)
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(opt_out_help, module)?)?;
    module.add_function(wrap_pyfunction!(validate_hook_sql_syntax, module)?)?;
    module.add_function(wrap_pyfunction!(validate_sql_syntax, module)?)?;
    module.add_function(wrap_pyfunction!(hook_sql_statements, module)?)
}
