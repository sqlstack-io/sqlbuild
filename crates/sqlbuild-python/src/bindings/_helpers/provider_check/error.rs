//! The `D012` error a run raises when its selection needs secrets that are missing.

use std::path::Path;

use pyo3::prelude::{PyErr, Python};
use sqlbuild_python_base::records::main::construct::new_annotated_error::new_annotated_error;
use sqlbuild_python_base::records::types::ErrorId;
use sqlbuild_python_front::discovery::main::sources::secret_places::secret_places;

use crate::bindings::_helpers::provider_check::selection::SecretBlock;

/// Set in a worker's environment; its run reports to the location's server.
const RUN_TOKEN_VARIABLE: &str = "SQLBUILD_RUN_TOKEN";

/// A node key's name, such as `raw_payments` for `source:raw_payments`.
fn short(key: &str) -> &str {
    key.split_once(':').map_or(key, |(_, name)| name)
}

/// One row per variable: provider, variable, and the nodes that use it and that read those.
fn rows(blocks: &[SecretBlock]) -> Vec<[String; 3]> {
    let mut rows: Vec<[String; 3]> = Vec::new();
    for block in blocks {
        let users: Vec<&str> = block.users.iter().map(|key| short(key)).collect();
        let mut needed_by: String = users.join(", ");
        if !block.readers.is_empty() {
            let readers: Vec<&str> = block.readers.iter().map(|key| short(key)).collect();
            needed_by.push_str(&format!("; read by {}", readers.join(", ")));
        }
        for variable in &block.variables {
            rows.push([
                block.provider.clone(),
                variable.clone(),
                format!("({needed_by})"),
            ]);
        }
    }
    rows
}

/// `D012`: the providers, variables and nodes, raised before any warehouse connection.
pub(crate) fn secret_block_error(
    py: Python<'_>,
    blocks: &[SecretBlock],
    project_dir: &Path,
) -> PyErr {
    let rows = rows(blocks);
    let width = |column: usize| {
        rows.iter()
            .map(|row| row[column].len())
            .max()
            .unwrap_or_default()
    };
    let (provider_width, variable_width) = (width(0), width(1));
    let lines: Vec<String> = rows
        .iter()
        .map(|[provider, variable, needed_by]| {
            format!("  {provider:<provider_width$}  {variable:<variable_width$}  {needed_by}")
        })
        .collect();
    let help: &str = if std::env::var_os(RUN_TOKEN_VARIABLE).is_some() {
        "set each variable in the location's environment (or its env file) and restart the location; the next run then works"
    } else {
        "set each variable in this shell or the project's env file, or narrow the selection"
    };
    new_annotated_error(
        py,
        ErrorId::ProviderDiscovery,
        format!(
            "selected nodes need secrets that are missing from {}:\n{}\nnothing was run",
            secret_places(project_dir),
            lines.join("\n")
        ),
        &[("help", help.to_owned())],
    )
}
