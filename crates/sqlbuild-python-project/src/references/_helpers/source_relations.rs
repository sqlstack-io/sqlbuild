//! A declared source rendered as the SQL table factor that reads it.

use sqlbuild_host::entries::models::AdapterHost;

use pyo3::prelude::{
    Bound, PyAny, PyAnyMethods, PyErr, PyModule, PyModuleMethods, PyResult, Python,
};
use pyo3::types::{PyDict, PyDictMethods, PyString};
use pyo3::{pyfunction, wrap_pyfunction};
use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::native_host::main::host_entry::host_entry;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// The SQL analysis dialect whose engines read replacing tables with `FINAL`.
const FINAL_READ_DIALECT: &str = "clickhouse";
/// Error code for a `final` source on another adapter.
const FINAL_UNSUPPORTED_CODE: &str = "A010";
/// Guidance for a `final` source on another adapter.
const FINAL_UNSUPPORTED_HELP: &str =
    "Remove 'final' from the source; only ClickHouse reads sources with FINAL.";

/// Leading words of a source expression that is a query rather than a relation.
const QUERY_PREFIXES: [&str; 3] = ["select", "with", "values"];

/// A source expression as a table factor: queries parenthesized, relations as written.
fn expression_relation(expression: &str) -> String {
    let trimmed: &str = expression.trim();
    let stripped: &str = trimmed.strip_suffix(';').unwrap_or(trimmed).trim();
    if stripped.starts_with('(') {
        return stripped.to_owned();
    }
    let lowered: String = stripped.to_lowercase();
    if QUERY_PREFIXES
        .iter()
        .any(|prefix| lowered.starts_with(prefix))
    {
        return format!("({stripped})");
    }
    stripped.to_owned()
}

/// Render a source as a SQL table factor, through the adapter when one is given.
#[pyfunction]
#[pyo3(signature = (*, entry, adapter = None))]
pub(crate) fn render_source_relation<'py>(
    entry: &Bound<'py, PyAny>,
    adapter: Option<&Bound<'py, PyAny>>,
) -> PyResult<String> {
    let adapter: Option<&Bound<'py, PyAny>> = adapter.filter(|adapter| !adapter.is_none());
    let expression: Option<String> = native_field(entry, "expression")?.extract()?;
    if let Some(expression) = expression {
        return match adapter {
            Some(adapter) => {
                let keywords: Bound<'py, PyDict> = PyDict::new(entry.py());
                keywords.set_item("expression", expression)?;
                adapter
                    .call_method("render_source_expression_relation", (), Some(&keywords))?
                    .extract()
            }
            None => Ok(expression_relation(&expression)),
        };
    }
    let table: Option<String> = native_field(entry, "table")?.extract()?;
    let table: String = match table {
        Some(table) => table,
        None => native_field(entry, "name")?.extract()?,
    };
    let database: Option<String> = native_field(entry, "database")?.extract()?;
    let schema: Option<String> = native_field(entry, "schema")?.extract()?;
    if let Some(adapter) = adapter {
        let keywords: Bound<'py, PyDict> = PyDict::new(entry.py());
        keywords.set_item("database", &database)?;
        keywords.set_item("schema", &schema)?;
        keywords.set_item("name", &table)?;
        let rendered: Option<String> = adapter
            .call_method("render_qualified_name", (), Some(&keywords))?
            .extract()?;
        if let Some(rendered) = rendered {
            return Ok(rendered);
        }
    }
    Ok([database, schema, Some(table)]
        .into_iter()
        .flatten()
        .collect::<Vec<String>>()
        .join("."))
}

/// A source as the table factor that reads it, deduplicated with `FINAL` when `final`.
pub(crate) fn render_source_read<'py>(
    entry: &Bound<'py, PyAny>,
    adapter: &Bound<'py, PyAny>,
) -> PyResult<String> {
    let relation: String = render_source_relation(entry, Some(adapter))?;
    if !native_field(entry, "final")?.is_truthy()? {
        return Ok(relation);
    }
    let dialect: Option<String> = native_field(adapter, "sql_analysis_dialect_name")?.extract()?;
    if dialect.as_deref() != Some(FINAL_READ_DIALECT) {
        return Err(final_unsupported(
            entry.py(),
            &native_field(adapter, "adapter_name")?.str()?.to_string(),
        ));
    }
    Ok(format!("(SELECT * FROM {relation} FINAL)"))
}

/// The coded error for a `final` source on an adapter that cannot read with `FINAL`.
fn final_unsupported(py: Python<'_>, adapter_name: &str) -> PyErr {
    match host_entry(py, AdapterHost::AdapterUserError).and_then(|class| {
        call_with(
            &class,
            &[
                (
                    "message",
                    PyString::new(
                        py,
                        &format!(
                            "adapter '{adapter_name}' does not support the source option 'final'"
                        ),
                    )
                    .into_any(),
                ),
                ("code", PyString::new(py, FINAL_UNSUPPORTED_CODE).into_any()),
                ("help", PyString::new(py, FINAL_UNSUPPORTED_HELP).into_any()),
            ],
        )
    }) {
        Ok(error) => PyErr::from_value(error),
        Err(error) => error,
    }
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(render_source_relation, module)?)?;
    Ok(())
}
