//! `sqb query`: run ad hoc SQL on the project connection and print the rows.

use std::time::Duration;

use pyo3::exceptions::PyException;
use pyo3::prelude::{
    Bound, PyAny, PyAnyMethods, PyErr, PyModule, PyModuleMethods, PyResult, Python,
};
use pyo3::types::{PyBool, PyDict, PyDictMethods, PyFloat, PyInt, PyString, PyStringMethods};
use pyo3::{pyfunction, wrap_pyfunction};
use sqlbuild_core::json::models::JsonValue;
use sqlbuild_core::progress::main::connection_start_message::connection_start_message;
use sqlbuild_core::query_command::main::render_query_json::render_query_json;
use sqlbuild_core::query_command::models::QueryDisplay;
use sqlbuild_host::entries::models::AdapterHost;

use crate::bindings::_helpers::boundary::python_calls::keywords;
use crate::bindings::_helpers::boundary::python_calls::{KeywordArguments, call_with};
use crate::bindings::_helpers::build_progress::transient_status::delayed_status;
use crate::bindings::_helpers::command_runtime::connection::resolve_project_connection_config;
use crate::bindings::_helpers::native_host::connections::connection_failure;
use crate::bindings::_helpers::native_host::host;
use crate::bindings::_helpers::query_command::project::{
    ProjectAdapter, ProjectInputs, project_adapter,
};
use crate::bindings::_helpers::query_command::query_values::{
    decimal_instance, display_text, json_cell, python_json_rows, strict_width,
};
use crate::bindings::_helpers::query_command::text_output::{LONG_FORMAT, render_text};
use crate::bindings::_helpers::records::construct::error_class;
use crate::bindings::_helpers::records::fields::field as native_field;
use crate::bindings::_helpers::records::ids::ErrorId;

const JSON_FORMAT: &str = "json";
const STATEMENT_OK: &str = "OK\n";
const DEFAULT_LIMIT: i64 = 20;
/// How long a step runs before its transient progress line appears.
const SLOW_STEP: Duration = Duration::from_millis(300);
const RUNNING_QUERY_MESSAGE: &str = "Running query...";

/// A `CliUserError` with its code and optional help.
fn user_error(py: Python<'_>, message: &str, code: Option<&str>, help: Option<&str>) -> PyErr {
    let raised = (|| {
        let options: Bound<'_, PyDict> = PyDict::new(py);
        if let Some(code) = code {
            options.set_item("code", code)?;
        }
        if let Some(help) = help {
            options.set_item("help", help)?;
        }
        error_class(py, ErrorId::CliUser)?.call((message,), Some(&options))
    })();
    match raised {
        Ok(error) => PyErr::from_value(error),
        Err(error) => error,
    }
}

fn empty_sql(py: Python<'_>) -> PyErr {
    user_error(py, "query SQL must not be empty", Some("C103"), None)
}

fn blank(text: &Bound<'_, PyAny>) -> PyResult<bool> {
    Ok(!text.call_method0("strip")?.is_truthy()?)
}

/// The SQL text of the file, refusing a missing path, a directory or non-UTF-8 text.
fn read_query_file<'py>(query_file: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = query_file.py();
    let shown: String = query_file.str()?.to_string_lossy().into_owned();
    if !query_file.call_method0("exists")?.is_truthy()? {
        return Err(user_error(
            py,
            &format!("query file does not exist: {shown}"),
            Some("C105"),
            None,
        ));
    }
    if !query_file.call_method0("is_file")?.is_truthy()? {
        return Err(user_error(
            py,
            &format!("query file path is not a file: {shown}"),
            Some("C106"),
            None,
        ));
    }
    let options: Bound<'py, PyDict> = PyDict::new(py);
    options.set_item("encoding", "utf-8")?;
    query_file
        .call_method("read_text", (), Some(&options))
        .map_err(|error| {
            let builtins = py.import("builtins");
            let readable: bool = builtins.is_ok_and(|builtins| {
                ["OSError", "UnicodeError"].iter().any(|name| {
                    builtins
                        .getattr(*name)
                        .is_ok_and(|class| error.value(py).is_instance(&class).unwrap_or(false))
                })
            });
            if !readable {
                return error;
            }
            let wrapped: PyErr = user_error(
                py,
                &format!("query file could not be read as UTF-8: {shown}"),
                Some("C107"),
                None,
            );
            wrapped.set_cause(py, Some(error));
            wrapped
        })
}

/// The SQL to run: the positional SQL or the file's text, never both and never blank.
fn query_sql<'py>(
    py: Python<'py>,
    sql: Option<Bound<'py, PyAny>>,
    query_file: Option<Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    let sql: Option<Bound<'py, PyAny>> = sql.filter(|value| !value.is_none());
    let query_file: Option<Bound<'py, PyAny>> = query_file.filter(|value| !value.is_none());
    let text: Bound<'py, PyAny> = match (sql, query_file) {
        (Some(_), Some(_)) => {
            return Err(user_error(
                py,
                "query accepts either positional SQL or --file, not both",
                Some("C104"),
                None,
            ));
        }
        (None, Some(path)) => read_query_file(&path)?,
        (Some(sql), None) => sql,
        (None, None) => {
            return Err(user_error(
                py,
                "query requires SQL",
                Some("C102"),
                Some("pass SQL as the query argument or use --file PATH"),
            ));
        }
    };
    if blank(&text)? {
        return Err(empty_sql(py));
    }
    Ok(text)
}

/// Connect, run the SQL and close; warehouse failures become `QueryExecutionError`.
fn run_sql<'py>(
    project: &ProjectAdapter<'py>,
    selected_target: Option<Bound<'py, PyAny>>,
    sql: Bound<'py, PyAny>,
    limit: Option<i64>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = sql.py();
    let connection_config: Bound<'py, PyAny> = resolve_project_connection_config(
        py,
        Some(keywords(
            py,
            &[
                ("discovered_inputs", project.discovered.clone()),
                ("project_dir", project.project_dir.clone()),
                (
                    "selected_target",
                    selected_target.unwrap_or_else(|| py.None().into_bound(py)),
                ),
            ],
        )?),
    )?;
    let adapter_name: String = native_field(&project.adapter, "adapter_name")?.extract()?;
    let connection: Bound<'py, PyAny> = delayed_status(
        py,
        (&connection_start_message(&adapter_name, 1), SLOW_STEP),
        || {
            project
                .adapter
                .call_method1("connect", (connection_config,))
                .map_err(|error| connection_failure(&project.adapter, &error))
        },
    )?;
    let limit_value: Bound<'py, PyAny> = match limit {
        Some(limit) => PyInt::new(py, limit).into_any(),
        None => py.None().into_bound(py),
    };
    let queried: PyResult<Bound<'py, PyAny>> =
        delayed_status(py, (RUNNING_QUERY_MESSAGE, SLOW_STEP), || {
            call_with(
                &native_field(&project.adapter, "query")?,
                &[
                    ("connection", connection.clone()),
                    ("sql", sql),
                    ("limit", limit_value),
                ],
            )
        });
    let queried: PyResult<Bound<'py, PyAny>> = queried.map_err(|error| {
        let adapter_error: bool = host::entry(py, AdapterHost::AdapterUserError)
            .is_ok_and(|class| error.value(py).is_instance(&class).unwrap_or(false));
        if adapter_error || !error.is_instance_of::<PyException>(py) {
            return error;
        }
        let value: Bound<'_, PyAny> = error.value(py).clone().into_any();
        let message: PyResult<Bound<'_, PyAny>> = match native_field(&value, "message") {
            Ok(message) => message.str().map(Bound::into_any),
            Err(_) => value.str().map(Bound::into_any),
        };
        let wrapped: PyErr = match message
            .and_then(|message| error_class(py, ErrorId::QueryExecution)?.call1((message,)))
        {
            Ok(raised) => PyErr::from_value(raised),
            Err(failure) => failure,
        };
        wrapped.set_cause(py, Some(error));
        wrapped
    });
    let closed: PyResult<Bound<'py, PyAny>> = project.adapter.call_method1("close", (connection,));
    let result: Bound<'py, PyAny> = queried?;
    closed?;
    Ok(result)
}

fn rows<'py>(result: &Bound<'py, PyAny>) -> PyResult<Vec<Vec<Bound<'py, PyAny>>>> {
    native_field(result, "rows")?
        .try_iter()?
        .map(|row| row?.try_iter()?.collect::<PyResult<Vec<_>>>())
        .collect()
}

fn texts(
    rows: &[Vec<Bound<'_, PyAny>>],
    convert: fn(&Bound<'_, PyAny>) -> PyResult<String>,
) -> PyResult<Vec<Vec<String>>> {
    rows.iter()
        .map(|row| row.iter().map(convert).collect())
        .collect()
}

fn render_json(
    result: &Bound<'_, PyAny>,
    columns: &[String],
    rows: &[Vec<Bound<'_, PyAny>>],
) -> PyResult<String> {
    let mut values: Vec<Vec<JsonValue>> = Vec::new();
    for row in rows {
        strict_width(columns.len(), row.len())?;
        let mut cells: Vec<JsonValue> = Vec::new();
        for value in row {
            match json_cell(value)? {
                Some(cell) => cells.push(cell),
                None => {
                    return python_json_rows(result.py(), &native_field(result, "columns")?, rows);
                }
            }
        }
        values.push(cells);
    }
    match render_query_json(columns, values) {
        Ok(rendered) => Ok(rendered),
        Err(_) => python_json_rows(result.py(), &native_field(result, "columns")?, rows),
    }
}

/// Whether every value of the column at `index` is a number or NULL, and at least one is a number.
fn numeric_column(rows: &[Vec<Bound<'_, PyAny>>], index: usize) -> PyResult<bool> {
    let mut seen: bool = false;
    for row in rows {
        let Some(value) = row.get(index) else {
            continue;
        };
        if value.is_none() {
            continue;
        }
        if !number(value)? {
            return Ok(false);
        }
        seen = true;
    }
    Ok(seen)
}

fn number(value: &Bound<'_, PyAny>) -> PyResult<bool> {
    if value.is_instance_of::<PyBool>() {
        return Ok(false);
    }
    Ok(value.is_instance_of::<PyInt>()
        || value.is_instance_of::<PyFloat>()
        || decimal_instance(value)?)
}

/// The column types the adapter reported, where it knows them.
fn column_types(result: &Bound<'_, PyAny>, width: usize) -> PyResult<Vec<Option<String>>> {
    let types: Vec<Option<String>> = native_field(result, "column_types")?
        .extract()
        .unwrap_or_default();
    Ok(if types.len() == width {
        types
    } else {
        Vec::new()
    })
}

/// The result in the requested format; a statement without columns prints `OK`.
fn render(
    result: &Bound<'_, PyAny>,
    output_format: Option<&str>,
    limit: Option<i64>,
) -> PyResult<String> {
    let py: Python<'_> = result.py();
    let columns: Vec<String> = native_field(result, "columns")?
        .try_iter()?
        .map(|column| column?.extract())
        .collect::<PyResult<_>>()?;
    if columns.is_empty() {
        return Ok(STATEMENT_OK.to_owned());
    }
    let rows: Vec<Vec<Bound<'_, PyAny>>> = rows(result)?;
    match output_format {
        Some(JSON_FORMAT) => render_json(result, &columns, &rows),
        None | Some(LONG_FORMAT) => {
            for row in &rows {
                strict_width(columns.len(), row.len())?;
            }
            let display = QueryDisplay {
                types: column_types(result, columns.len())?,
                numeric: (0..columns.len())
                    .map(|index| numeric_column(&rows, index))
                    .collect::<PyResult<_>>()?,
                columns,
                rows: texts(&rows, display_text)?,
                truncated: native_field(result, "truncated")?.is_truthy()?,
                limit,
            };
            render_text(py, &display, output_format)
        }
        Some(other) => Err(user_error(
            py,
            &format!("unsupported query output format '{other}'"),
            None,
            None,
        )),
    }
}

/// Execute ad hoc SQL against the active project connection.
#[pyfunction]
#[pyo3(signature = (**options))]
pub(crate) fn run_query_command(
    py: Python<'_>,
    options: Option<Bound<'_, PyDict>>,
) -> PyResult<i32> {
    let arguments: KeywordArguments<'_> = KeywordArguments::read(
        py,
        "run_query_command",
        options,
        (
            &["project_dir", "sql"],
            &["query_file", "selected_target", "output_format", "limit"],
        ),
    )?;
    let output_format: Option<String> = if arguments.passed("output_format")? {
        arguments.value("output_format")?.extract()?
    } else {
        None
    };
    let limit: Option<i64> = if arguments.passed("limit")? {
        arguments.value("limit")?.extract()?
    } else {
        Some(DEFAULT_LIMIT)
    };
    let sql: Bound<'_, PyAny> = query_sql(
        py,
        Some(arguments.value("sql")?),
        Some(arguments.value("query_file")?),
    )?;
    if limit.is_some_and(|limit| limit < 0) {
        return Err(user_error(
            py,
            "query --limit must be greater than or equal to 0",
            Some("C101"),
            Some("pass a non-negative integer or use --no-limit"),
        ));
    }
    let project: ProjectAdapter<'_> = project_adapter(
        py,
        Some(arguments.value("project_dir")?),
        ProjectInputs::Configuration,
    )?;
    let result: Bound<'_, PyAny> = run_sql(
        &project,
        Some(arguments.value("selected_target")?),
        sql,
        limit,
    )?;
    let rendered: String = render(&result, output_format.as_deref(), limit)?;
    let stdout: Bound<'_, PyAny> = py.import("sys")?.getattr("stdout")?;
    stdout.call_method1("write", (PyString::new(py, &rendered),))?;
    stdout.call_method0("flush")?;
    Ok(0)
}

/// Render a query result using the requested output format.
#[pyfunction]
#[pyo3(signature = (*, result, output_format, limit))]
fn render_query_result(
    result: &Bound<'_, PyAny>,
    output_format: Option<&str>,
    limit: Option<i64>,
) -> PyResult<String> {
    render(result, output_format, limit)
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(run_query_command, module)?)?;
    module.add_function(wrap_pyfunction!(render_query_result, module)?)?;
    Ok(())
}
