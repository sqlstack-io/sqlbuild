//! The note counting the models SQBRPERFORMANCE102 skipped, listing them when verbose.

/// The note for `skipped` `(path, reason)` pairs, or `None` when no model was skipped.
pub fn format_skipped_models(skipped: &[(String, String)], verbose: bool) -> Option<String> {
    if skipped.is_empty() {
        return None;
    }
    let mut note: String = format!(
        "SQBRPERFORMANCE102 skipped {} model{} without column lineage or inferred types",
        skipped.len(),
        if skipped.len() == 1 { "" } else { "s" }
    );
    if verbose {
        for (path, reason) in skipped {
            note.push_str(&format!("\n  {path}: {reason}"));
        }
    } else {
        note.push_str("; `sqb plan --verbose` lists them");
    }
    Some(note)
}
