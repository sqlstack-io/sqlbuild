//! A provider class's setting fields and which of its secret settings the environment leaves unset.

use std::cell::Cell;

use pyo3::exceptions::PyValueError;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};

use sqlbuild_python_base::orchestration_reads::main::missing_secrets::missing_secrets;
use sqlbuild_python_base::records::main::construct::new_annotated_error::new_annotated_error;
use sqlbuild_python_base::records::main::fields::field::field;
use sqlbuild_python_base::records::types::ErrorId;

/// Discovered collections of sinks that run with every command.
const SINK_COLLECTIONS: [&str; 2] = ["event_exporters", "command_output_sinks"];

thread_local! {
    /// Whether the invocation running on this thread passed `--allow-missing-secrets`.
    static INVOCATION_ALLOWS_MISSING_SECRETS: Cell<bool> = const { Cell::new(false) };
}

/// `--allow-missing-secrets` for one invocation, restored when the invocation ends.
pub(crate) struct MissingSecretsScope {
    previous: bool,
}

impl MissingSecretsScope {
    pub(crate) fn enter(allowed: bool) -> Self {
        Self {
            previous: INVOCATION_ALLOWS_MISSING_SECRETS.replace(allowed),
        }
    }
}

impl Drop for MissingSecretsScope {
    fn drop(&mut self) {
        INVOCATION_ALLOWS_MISSING_SECRETS.set(self.previous);
    }
}

/// Whether the running invocation passed `--allow-missing-secrets`.
pub(crate) fn invocation_allows_missing_secrets() -> bool {
    INVOCATION_ALLOWS_MISSING_SECRETS.get()
}

/// What discovery does when a provider's required secrets are unset.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SecretPolicy {
    /// Fail with D012: static commands by default.
    Require,
    /// Continue and list the providers on stderr: static commands that opted out.
    Allow,
    /// Continue silently; the command checks the providers it uses before running anything.
    Defer,
}

impl SecretPolicy {
    /// The keyword value that selects the policy.
    pub(crate) const fn name(self) -> &'static str {
        match self {
            Self::Require => "require",
            Self::Allow => "allow",
            Self::Defer => "defer",
        }
    }

    pub(crate) fn parse(value: &str) -> PyResult<Self> {
        [Self::Require, Self::Allow, Self::Defer]
            .into_iter()
            .find(|policy| policy.name() == value)
            .ok_or_else(|| PyValueError::new_err(format!("unknown secret policy '{value}'")))
    }
}

/// `(provider, consumer)` for each usage, named by the usage's own `consumer_name` or else by `consumer`.
pub(crate) fn provider_uses(
    usages: &Bound<'_, PyAny>,
    consumer: Option<&str>,
) -> PyResult<Vec<(String, String)>> {
    usages
        .try_iter()?
        .map(|usage| usage_pair(&usage?, consumer))
        .collect()
}

fn usage_pair(usage: &Bound<'_, PyAny>, consumer: Option<&str>) -> PyResult<(String, String)> {
    let provider: String = field(usage, "provider_name")?.extract()?;
    let name: String = match consumer {
        Some(name) => name.to_owned(),
        None => field(usage, "consumer_name")?.extract()?,
    };
    Ok((provider, name))
}

/// `(provider, consumer)` for each provider the named consumers, such as checks or loaders, inject.
pub(crate) fn consumer_uses(consumers: &Bound<'_, PyAny>) -> PyResult<Vec<(String, String)>> {
    let mut uses: Vec<(String, String)> = Vec::new();
    for consumer in consumers.try_iter()? {
        let consumer: Bound<'_, PyAny> = consumer?;
        let name: String = field(&consumer, "name")?.extract()?;
        uses.extend(provider_uses(
            &field(&consumer, "provider_usages")?,
            Some(&name),
        )?);
    }
    Ok(uses)
}

/// The providers the project's lifecycle and command-output sinks use, which run with every command.
pub(crate) fn sink_uses(discovered: &Bound<'_, PyAny>) -> PyResult<Vec<(String, String)>> {
    let mut uses: Vec<(String, String)> = Vec::new();
    for collection in SINK_COLLECTIONS {
        if let Some(sinks) = discovered.getattr_opt(collection)? {
            uses.extend(consumer_uses(&sinks)?);
        }
    }
    Ok(uses)
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
