//! Refusals grouped by code and construct, then by where the construct is written.

use std::cmp::Reverse;
use std::path::Path;

use crate::run_on::models::{LocalRefusal, OriginUser, RefusalGroup, RefusalOrigin, RunOnDecision};

/// The group a refusal belongs to: its code and construct; its category when it has no place or names its item.
fn group_key(refusal: &LocalRefusal, item: &str) -> (String, String, bool) {
    let code: String = refusal.code.clone().unwrap_or_default();
    if refusal.path.is_none() || names_item(refusal, item) {
        return (code, refusal.category.clone(), true);
    }
    (code, refusal.construct.clone(), false)
}

/// Whether the construct is the item itself, as a stale snapshot's `scenario:<name>` is.
pub(crate) fn names_item(refusal: &LocalRefusal, item: &str) -> bool {
    refusal.construct.rsplit(':').next() == Some(item)
}

/// Where in its group a refusal is written: its macro, its file position, or its construct.
fn origin_key(refusal: &LocalRefusal) -> String {
    if let Some(name) = &refusal.macro_name {
        return format!("macro {name}");
    }
    match &refusal.path {
        Some(path) => format!(
            "{path}:{}:{}",
            refusal.line.unwrap_or(0),
            refusal.column.unwrap_or(0)
        ),
        None => refusal.construct.clone(),
    }
}

/// The model a refusal's place is in: its file's stem.
fn model_name(refusal: &LocalRefusal) -> String {
    refusal
        .path
        .as_deref()
        .and_then(|path| Path::new(path).file_stem())
        .map_or_else(String::new, |stem| stem.to_string_lossy().into_owned())
}

fn with_unique(mut items: Vec<String>, item: &str) -> Vec<String> {
    if !items.iter().any(|seen| seen == item) {
        items.push(item.to_owned());
    }
    items
}

/// Every refused construct once, most items first, each with its origins in first-seen order.
pub(crate) fn group_refusals(decisions: &[&RunOnDecision]) -> Vec<RefusalGroup> {
    let mut keys: Vec<(String, String, bool)> = Vec::new();
    let mut groups: Vec<RefusalGroup> = Vec::new();
    for decision in decisions {
        for refusal in &decision.refusals {
            let key: (String, String, bool) = group_key(refusal, &decision.name);
            let index: usize = match keys.iter().position(|seen| *seen == key) {
                Some(index) => index,
                None => {
                    keys.push(key.clone());
                    groups.push(RefusalGroup {
                        refusal: refusal.clone(),
                        unplaced: key.2,
                        ..RefusalGroup::default()
                    });
                    groups.len() - 1
                }
            };
            let group: &mut RefusalGroup = &mut groups[index];
            group.items = with_unique(std::mem::take(&mut group.items), &decision.name);
            let origin: String = origin_key(refusal);
            let at: usize = match group
                .origins
                .iter()
                .position(|seen| origin_key(&seen.refusal) == origin)
            {
                Some(at) => at,
                None => {
                    group.origins.push(RefusalOrigin {
                        refusal: refusal.clone(),
                        ..RefusalOrigin::default()
                    });
                    group.origins.len() - 1
                }
            };
            let entry: &mut RefusalOrigin = &mut group.origins[at];
            entry.items = with_unique(std::mem::take(&mut entry.items), &decision.name);
            let model: String = model_name(refusal);
            if model.is_empty() {
                continue;
            }
            match entry.users.iter_mut().find(|user| user.model == model) {
                Some(user) => {
                    user.items = with_unique(std::mem::take(&mut user.items), &decision.name)
                }
                None => entry.users.push(OriginUser {
                    model,
                    line: refusal.line.unwrap_or(0),
                    items: vec![decision.name.clone()],
                }),
            }
        }
    }
    for group in &mut groups {
        group.origins.sort_by_key(|item| Reverse(item.items.len()));
        for origin in &mut group.origins {
            origin.users.sort_by_key(|item| Reverse(item.items.len()));
        }
    }
    groups.sort_by_key(|item| Reverse(item.items.len()));
    groups
}
