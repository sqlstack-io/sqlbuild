//! The `D012` error a run raises when its selection needs secrets that are missing.

use std::path::Path;

use pyo3::prelude::{PyErr, Python};
use sqlbuild_python_base::records::main::construct::new_annotated_error::new_annotated_error;
use sqlbuild_python_base::records::types::ErrorId;
use sqlbuild_python_front::discovery::main::sources::secret_places::secret_places;

use crate::provider_check::_helpers::text::{rows, sink_lines};
use crate::provider_check::constants::RUN_TOKEN_VARIABLE;
use crate::provider_check::models::SecretBlock;

/// `D012`: the providers, variables and nodes, raised before any warehouse connection.
pub fn secret_block_error(py: Python<'_>, blocks: &[SecretBlock], project_dir: &Path) -> PyErr {
    let rows = rows(blocks);
    let width = |column: usize| {
        rows.iter()
            .map(|row| row[column].len())
            .max()
            .unwrap_or_default()
    };
    let (provider_width, variable_width) = (width(0), width(1));
    let mut lines: Vec<String> = rows
        .iter()
        .map(|[provider, variable, needed_by]| {
            format!("  {provider:<provider_width$}  {variable:<variable_width$}  {needed_by}")
        })
        .collect();
    lines.extend(
        sink_lines(blocks)
            .into_iter()
            .map(|line| format!("  {line}")),
    );
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
