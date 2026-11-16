//! A provider class's setting fields and which of its secret settings the environment leaves unset.

use std::cell::Cell;

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};

use sqlbuild_python_base::orchestration_reads::main::missing_secrets::missing_secrets;
use sqlbuild_python_base::records::main::construct::new_annotated_error::new_annotated_error;
use sqlbuild_python_base::records::main::fields::field::field;
use sqlbuild_python_base::records::types::ErrorId;

/// Discovered collections of sinks that run with every command.
pub(crate) const SINK_COLLECTIONS: [&str; 2] = ["event_exporters", "command_output_sinks"];

thread_local! {
    /// Whether the invocation running on this thread passed `--allow-missing-secrets`.
    pub(crate) static INVOCATION_ALLOWS_MISSING_SECRETS: Cell<bool> = const { Cell::new(false) };
}

pub(crate) fn usage_pair(
    usage: &Bound<'_, PyAny>,
    consumer: Option<&str>,
) -> PyResult<(String, String)> {
    let provider: String = field(usage, "provider_name")?.extract()?;
    let name: String = match consumer {
        Some(name) => name.to_owned(),
        None => field(usage, "consumer_name")?.extract()?,
    };
    Ok((provider, name))
}

/// Fail before anything runs when a provider in `uses` lacks required secrets, listing each with what needs it.
pub(crate) fn require_used_secrets(
    discovered: &Bound<'_, PyAny>,
    uses: &[(String, String)],
) -> PyResult<()> {
    let py: Python<'_> = discovered.py();
    let mut consumers: Vec<(String, Vec<String>)> = Vec::new();
    for (provider, consumer) in uses {
        match consumers.iter_mut().find(|(name, _)| name == provider) {
            Some((_, names)) if !names.contains(consumer) => names.push(consumer.clone()),
            Some(_) => {}
            None => consumers.push((provider.clone(), vec![consumer.clone()])),
        }
    }
    let mut rows: Vec<[String; 3]> = Vec::new();
    for provider in field(discovered, "providers")?.try_iter()? {
        let provider: Bound<'_, PyAny> = provider?;
        let name: String = field(&provider, "name")?.extract()?;
        let Some((_, names)) = consumers.iter().find(|(used, _)| *used == name) else {
            continue;
        };
        let needed_by: String = match names.as_slice() {
            [only] => only.clone(),
            [first, rest @ ..] => format!("{first}, +{}", rest.len()),
            [] => String::new(),
        };
        for variable in missing_secrets(&field(&provider, "provider_class")?)? {
            rows.push([name.clone(), variable, format!("({needed_by})")]);
        }
    }
    if rows.is_empty() {
        return Ok(());
    }
    let widths: [usize; 2] = [0, 1].map(|column| column_width(&rows, column));
    let lines: Vec<String> = rows
        .iter()
        .map(|[provider, variable, needed_by]| {
            format!(
                "  {provider:<provider_width$}  {variable:<variable_width$}  {needed_by}",
                provider_width = widths[0],
                variable_width = widths[1],
            )
        })
        .collect();
    Err(new_annotated_error(
        py,
        ErrorId::ProviderDiscovery,
        format!(
            "selected nodes need secrets that are missing:\n{}\nnothing was run",
            lines.join("\n")
        ),
        &[(
            "help",
            "set each variable, or narrow the selection".to_owned(),
        )],
    ))
}

fn column_width(rows: &[[String; 3]], column: usize) -> usize {
    rows.iter()
        .map(|row| row[column].len())
        .max()
        .unwrap_or_default()
}
