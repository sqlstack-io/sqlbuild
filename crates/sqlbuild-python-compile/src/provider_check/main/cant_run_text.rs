//! The text plan's `Can't run` lines.

use crate::provider_check::_helpers::text::names;
use crate::provider_check::models::SecretBlock;

/// Per provider, its variables and the nodes it stops, after a blank line.
pub fn cant_run_text(blocks: &[SecretBlock], missing_from: &str) -> String {
    let mut lines: Vec<String> = Vec::new();
    for block in blocks {
        lines.push(format!(
            "Can't run: provider {} needs {} (missing from {missing_from})",
            block.provider,
            block.variables.join(", ")
        ));
        let mut needs: String = names(&block.users);
        if !block.readers.is_empty() {
            needs.push_str(&format!(", and {} that read it", names(&block.readers)));
        }
        lines.push(format!("  needs  {needs}"));
    }
    format!("\n{}", lines.join("\n"))
}
