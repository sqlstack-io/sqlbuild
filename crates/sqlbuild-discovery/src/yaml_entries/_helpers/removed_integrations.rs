//! The removed declarative integrations: a source's `ingestr` block and top-level `dlt_sources`.

use sqlbuild_model_config::errors::ConfigError;
use sqlbuild_model_config::types::AuthoredNode;

use crate::yaml_entries::_helpers::fields::{Site, get};

const LOADER_HELP: &str = "with `managed: true` and load it with a Python `@loader` named after \
                           the source instead";

/// A source that still declares an `ingestr` block, which SQLBuild no longer runs.
pub(crate) fn reject_ingestr_source<N: AuthoredNode>(
    entry: &[(N, N)],
    site: Site<'_>,
) -> Result<(), ConfigError> {
    if get(entry, "ingestr").is_none() {
        return Ok(());
    }
    let name: String = get(entry, "name")
        .and_then(AuthoredNode::text)
        .map_or_else(String::new, |name| format!("'{name}' "));
    Err(site.error(&format!(
        "{name}declares an ingestr integration, which SQLBuild no longer supports; declare the \
         source {LOADER_HELP}"
    )))
}

/// A sources file that still declares `dlt_sources`, which SQLBuild no longer runs.
pub(crate) fn reject_dlt_sources<N: AuthoredNode>(
    payload: &[(N, N)],
    path: &str,
) -> Result<(), ConfigError> {
    if get(payload, "dlt_sources").is_none() {
        return Ok(());
    }
    Err(ConfigError::compile(format!(
        "{path} declares dlt_sources, a dlt integration, which SQLBuild no longer supports; \
         declare each source under `sources:` {LOADER_HELP}"
    )))
}
