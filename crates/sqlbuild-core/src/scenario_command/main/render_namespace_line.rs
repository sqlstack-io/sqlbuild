//! The dim line under a scenario run's header that names its namespace.

use crate::cli_text::models::CliStyle;
use crate::scenario_command::models::ScenarioNamespace;

/// `  namespace  <value> (from <source>)` when the run is namespaced; empty when it isn't.
#[must_use]
pub fn render_namespace_line(namespace: &ScenarioNamespace, style: CliStyle) -> String {
    namespace.value.as_ref().map_or_else(String::new, |value| {
        format!(
            "{}\n",
            style.muted(&format!("  namespace  {value} (from {})", namespace.source))
        )
    })
}
