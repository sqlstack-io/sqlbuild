//! The lines naming upstreams of a missing-upstream message that one target lacks.

use crate::planner_identity::constants::MISSING_MESSAGE_ITEM_LIMIT;

/// How many upstreams of these kinds are missing from `target`, a few by name.
pub(crate) fn missing_lines(missing: &[(&str, &str)], target: Option<&String>) -> Vec<String> {
    let one: bool = missing.len() == 1;
    let kinds: std::collections::BTreeSet<&str> = missing.iter().map(|(_, kind)| *kind).collect();
    let noun: String = match kinds.iter().collect::<Vec<_>>().as_slice() {
        [kind] if one => (*kind).to_string(),
        [kind] => format!("{kind}s"),
        _ => "models and seeds".to_owned(),
    };
    let place: String = target.map_or_else(
        || "the warehouse".to_owned(),
        |target| format!("target '{target}'"),
    );
    let mut lines: Vec<String> = vec![format!(
        "{} upstream {noun} this build reads {} exist in {place}",
        missing.len(),
        if one { "doesn't" } else { "don't" }
    )];
    lines.extend(
        missing
            .iter()
            .take(MISSING_MESSAGE_ITEM_LIMIT)
            .map(|(name, _)| format!("  {name}")),
    );
    if missing.len() > MISSING_MESSAGE_ITEM_LIMIT {
        lines.push(format!(
            "  ... {} more (sqb plan --json lists all)",
            missing.len() - MISSING_MESSAGE_ITEM_LIMIT
        ));
    }
    lines
}
