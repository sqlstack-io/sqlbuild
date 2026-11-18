//! The error message of a selected scope with missing upstreams.

use std::collections::HashMap;

use crate::planner_identity::constants::MISSING_MESSAGE_ITEM_LIMIT;
use crate::planner_identity::models::{GraphKey, MissingUpstream, MissingUpstreamFixes};

/// Missing lineage upstreams with how to fix them and, when only an audit or hook read requires one, that read's origin.
pub fn missing_upstream_message(
    missing: &[MissingUpstream],
    edge_origins: &HashMap<(GraphKey, GraphKey), String>,
    fixes: &MissingUpstreamFixes,
) -> String {
    let mut lineage_missing: Vec<&str> = Vec::new();
    let mut kinds: std::collections::BTreeSet<&str> = std::collections::BTreeSet::new();
    let mut read_names: Vec<&str> = Vec::new();
    for entry in missing {
        let origins: Vec<&str> = entry
            .required_by
            .iter()
            .filter_map(|dependent| {
                edge_origins
                    .get(&(dependent.clone(), entry.key.clone()))
                    .map(String::as_str)
            })
            .collect();
        if origins.len() < entry.required_by.len() {
            lineage_missing.push(entry.key.name.as_str());
            kinds.insert(entry.key.kind.as_str());
            continue;
        }
        for origin in origins {
            if !read_names.contains(&origin) {
                read_names.push(origin);
            }
        }
    }
    let reads: Option<String> = audit_reads_text(&read_names);
    if lineage_missing.is_empty() {
        return format!("cannot build selected scope: {}", reads.unwrap_or_default());
    }
    let mut text: String = lineage_text(&lineage_missing, &kinds, fixes);
    if let Some(reads) = reads {
        text.push_str(&format!("\n\n  Also, {reads}"));
    }
    text
}

/// The missing lineage upstreams, a few by name, then how to read, clone or build them.
fn lineage_text(
    names: &[&str],
    kinds: &std::collections::BTreeSet<&str>,
    fixes: &MissingUpstreamFixes,
) -> String {
    let one: bool = names.len() == 1;
    let noun: String = match kinds.iter().collect::<Vec<_>>().as_slice() {
        [kind] if one => (*kind).to_string(),
        [kind] => format!("{kind}s"),
        _ => "models and seeds".to_owned(),
    };
    let place: String = fixes.target.as_ref().map_or_else(
        || "the warehouse".to_owned(),
        |target| format!("target '{target}'"),
    );
    let mut lines: Vec<String> = vec![format!(
        "{} upstream {noun} this build reads {} exist in {place}",
        names.len(),
        if one { "doesn't" } else { "don't" }
    )];
    lines.extend(
        names
            .iter()
            .take(MISSING_MESSAGE_ITEM_LIMIT)
            .map(|name| format!("  {name}")),
    );
    if names.len() > MISSING_MESSAGE_ITEM_LIMIT {
        lines.push(format!(
            "  ... {} more (sqb plan --json lists all)",
            names.len() - MISSING_MESSAGE_ITEM_LIMIT
        ));
    }
    lines.push(String::new());
    let (exists, them): (&str, &str) = if one {
        ("it exists", "it")
    } else {
        ("they exist", "them")
    };
    let read_label: String = format!("read {them} from it for this run:");
    let copy_label: String = format!("copy {them} into this target:");
    let build_label: String = format!(
        "{} {them} too:",
        if fixes.origins.is_empty() {
            "Build"
        } else {
            "Or build"
        }
    );
    let width: usize = if fixes.origins.is_empty() {
        build_label.len() + 2
    } else {
        (read_label.len() + 4).max(copy_label.len() + 4)
    }
    .max(build_label.len() + 2);
    let to: String = fixes
        .clone_to
        .as_ref()
        .map(|target| format!(" --to {target}"))
        .unwrap_or_default();
    for origin in &fixes.origins {
        lines.push(format!("  If {exists} in {origin}:"));
        if fixes.deferrable.contains(origin) {
            lines.push(format!(
                "    {read_label:<0$}  --defer-to {origin}",
                width - 4
            ));
        }
        lines.push(format!(
            "    {copy_label:<0$}  sqb clone --from {origin}{to} -s {1}",
            width - 4,
            names.join(" ")
        ));
    }
    lines.push(format!(
        "  {build_label:<0$}  add + before your selection",
        width - 2
    ));
    lines.join("\n")
}

/// The audit and hook reads that need a missing upstream, a few by name.
fn audit_reads_text(read_names: &[&str]) -> Option<String> {
    match read_names {
        [] => None,
        [name] => Some(format!(
            "{name}, which is not selected and does not exist in the warehouse; select it or build it first"
        )),
        _ => {
            let shown: String = read_names
                .iter()
                .take(MISSING_MESSAGE_ITEM_LIMIT)
                .copied()
                .collect::<Vec<&str>>()
                .join("; ");
            let more: &str = if read_names.len() > MISSING_MESSAGE_ITEM_LIMIT {
                "; ..."
            } else {
                ""
            };
            Some(format!(
                "{} audit reads are not selected and do not exist in the warehouse ({shown}{more}); select them or build them first",
                read_names.len()
            ))
        }
    }
}
