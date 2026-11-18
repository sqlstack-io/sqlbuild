//! A unified diff's added and removed lines, whether they differ only in whitespace, and a short config change in words.

use crate::plan_display::constants::{
    CONFIG_CHANGE_KEYS, CONFIG_TOP_LEVEL_INDENT, DIFF_HEADER_PREFIXES, UNSET_CONFIG_VALUE,
};
use crate::plan_display::models::DiffStats;

/// The diff's changed lines; whitespace-only when they match with all whitespace out, so case changes count.
pub(crate) fn diff_stats(diff: &[String]) -> DiffStats {
    let mut removed: Vec<&str> = Vec::new();
    let mut added: Vec<&str> = Vec::new();
    let mut in_hunk = false;
    for line in diff {
        if line.starts_with("@@") {
            in_hunk = true;
            continue;
        }
        if !in_hunk
            && DIFF_HEADER_PREFIXES
                .iter()
                .any(|prefix| line.starts_with(prefix))
        {
            continue;
        }
        if let Some(text) = line.strip_prefix('-') {
            removed.push(text);
        } else if let Some(text) = line.strip_prefix('+') {
            added.push(text);
        }
    }
    let changed = !removed.is_empty() || !added.is_empty();
    DiffStats {
        added: added.len(),
        removed: removed.len(),
        whitespace_only: changed && without_whitespace(&removed) == without_whitespace(&added),
    }
}

fn without_whitespace(lines: &[&str]) -> String {
    lines
        .iter()
        .flat_map(|line| line.chars())
        .filter(|character| !character.is_whitespace())
        .collect()
}

/// A config diff's changes in one line, such as `full_refresh true → false`, when it changes at most two top-level keys with plain values.
pub(crate) fn config_change(diff: &[String]) -> Option<String> {
    let mut values: Vec<(&str, Option<&str>, Option<&str>)> = Vec::new();
    for line in diff {
        let removed = line.starts_with('-');
        if DIFF_HEADER_PREFIXES
            .iter()
            .any(|prefix| line.starts_with(prefix))
            || !(removed || line.starts_with('+'))
        {
            continue;
        }
        let (key, value) = config_value(&line[1..])?;
        let at = match values.iter().position(|(known, _, _)| *known == key) {
            Some(at) => at,
            None => {
                values.push((key, None, None));
                values.len() - 1
            }
        };
        if removed {
            values[at].1 = Some(value);
        } else {
            values[at].2 = Some(value);
        }
    }
    values.retain(|(_, old, new)| old != new);
    if values.is_empty() || values.len() > CONFIG_CHANGE_KEYS {
        return None;
    }
    let changes: Vec<String> = values
        .into_iter()
        .map(|(key, old, new)| {
            format!(
                "{key} {} → {}",
                old.unwrap_or(UNSET_CONFIG_VALUE),
                new.unwrap_or(UNSET_CONFIG_VALUE)
            )
        })
        .collect();
    Some(changes.join("; "))
}

/// One top-level `"key": value` line's key and plain value, the config printed with two-space indents; `None` for a nested key or value, or any other line.
fn config_value(line: &str) -> Option<(&str, &str)> {
    let (key, rest) = line
        .strip_prefix(CONFIG_TOP_LEVEL_INDENT)?
        .strip_prefix('"')?
        .split_once('"')?;
    let value = rest.strip_prefix(':')?.trim();
    let value = value.strip_suffix(',').unwrap_or(value).trim_end();
    if key.is_empty() || value.ends_with('{') || value.ends_with('[') {
        return None;
    }
    let plain = value
        .strip_prefix('"')
        .and_then(|inner| inner.strip_suffix('"'))
        .unwrap_or(value);
    Some((key, plain))
}
