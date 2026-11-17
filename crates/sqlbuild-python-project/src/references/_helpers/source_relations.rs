//! A declared source rendered as the SQL table factor that reads it.

use sqlbuild_host::entries::models::AdapterHost;
use sqlbuild_python_base::records::main::construct::exception_from::exception_from;

use pyo3::prelude::{PyErr, Python};
use sqlbuild_python_base::native_host::main::host_entry::host_entry;

/// The SQL analysis dialect whose engines read replacing tables with `FINAL`.
pub(crate) const FINAL_READ_DIALECT: &str = "clickhouse";
/// Error code for a `final` source on another adapter.
const FINAL_UNSUPPORTED_CODE: &str = "A010";
/// Guidance for a `final` source on another adapter.
const FINAL_UNSUPPORTED_HELP: &str =
    "Remove 'final' from the source; only ClickHouse reads sources with FINAL.";

/// Leading words of a source expression that is a query rather than a relation.
const QUERY_PREFIXES: [&str; 3] = ["select", "with", "values"];

/// A source expression as a table factor: queries parenthesized, relations as written.
pub(crate) fn expression_relation(expression: &str) -> String {
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

/// The coded error for a `final` source on an adapter that cannot read with `FINAL`.
pub(crate) fn final_unsupported(py: Python<'_>, adapter_name: &str) -> PyErr {
    exception_from(
        host_entry(py, AdapterHost::AdapterUserError),
        &format!("adapter '{adapter_name}' does not support the source option 'final'"),
        (Some(FINAL_UNSUPPORTED_CODE), Some(FINAL_UNSUPPORTED_HELP)),
    )
}
