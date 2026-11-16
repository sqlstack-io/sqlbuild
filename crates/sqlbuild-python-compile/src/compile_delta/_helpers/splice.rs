//! Splice a delta compile's JSON report into the stored report it patches.

use crate::compile_delta::models::SpliceScope;
use std::collections::HashMap;

use serde_json::Value;

/// One JSON value's byte span in its document.
type Span = (usize, usize);

/// Bytes per word `string_stop` checks at once.
const WORD: usize = 8;
const LOW_BITS: u64 = 0x0101_0101_0101_0101;
const HIGH_BITS: u64 = 0x8080_8080_8080_8080;

const DELTA_SUMMARY_KEYS: [&str; 2] = ["errors", "warnings"];

/// Whether any byte of `word` equals `byte`.
fn has_byte(word: u64, byte: u8) -> bool {
    let matched: u64 = word ^ (LOW_BITS * u64::from(byte));
    matched.wrapping_sub(LOW_BITS) & !matched & HIGH_BITS != 0
}

/// The first quote or backslash at or after `index`, or the end, skipping whole clean words.
fn string_stop(text: &[u8], mut index: usize) -> usize {
    while index + WORD <= text.len() {
        let mut word: [u8; WORD] = [0; WORD];
        word.copy_from_slice(&text[index..index + WORD]);
        let word: u64 = u64::from_le_bytes(word);
        if has_byte(word, b'"') || has_byte(word, b'\\') {
            break;
        }
        index += WORD;
    }
    while index < text.len() && !matches!(text[index], b'"' | b'\\') {
        index += 1;
    }
    index
}

/// The end of the JSON value starting at `start`, skipping strings and nested containers.
fn value_end(text: &[u8], start: usize) -> Option<usize> {
    let mut depth: usize = 0;
    let mut index: usize = start;
    let mut in_string: bool = false;
    while index < text.len() {
        let byte: u8 = text[index];
        if in_string {
            index = string_stop(text, index);
            let Some(&byte) = text.get(index) else {
                break;
            };
            match byte {
                b'\\' => index += 1,
                b'"' => {
                    in_string = false;
                    if depth == 0 {
                        return Some(index + 1);
                    }
                }
                _ => {}
            }
        } else {
            match byte {
                b'"' => in_string = true,
                b'{' | b'[' => depth += 1,
                b'}' | b']' => {
                    if depth == 0 {
                        return Some(index);
                    }
                    depth -= 1;
                    if depth == 0 {
                        return Some(index + 1);
                    }
                }
                b',' | b'\n' if depth == 0 => return Some(index),
                _ => {}
            }
        }
        index += 1;
    }
    (depth == 0 && !in_string).then_some(index)
}

fn skip_space(text: &[u8], mut index: usize) -> usize {
    while index < text.len() && matches!(text[index], b' ' | b'\n' | b'\r' | b'\t') {
        index += 1;
    }
    index
}

/// The value `text` holds as JSON, or `None` when it is malformed.
fn from_json<T: serde::de::DeserializeOwned>(text: &str) -> Option<T> {
    let Ok(value) = serde_json::from_str(text) else {
        return None;
    };
    Some(value)
}

/// The `(key, value span)` members of the object spanning `span`.
fn members(text: &str, span: Span) -> Option<Vec<(String, Span)>> {
    let bytes: &[u8] = text.as_bytes();
    if bytes.get(span.0) != Some(&b'{') {
        return None;
    }
    let mut found: Vec<(String, Span)> = Vec::new();
    let mut index: usize = skip_space(bytes, span.0 + 1);
    while index < span.1 && bytes[index] != b'}' {
        let key_end: usize = value_end(bytes, index)?;
        let key: String = from_json(&text[index..key_end])?;
        index = skip_space(bytes, key_end);
        if bytes.get(index) != Some(&b':') {
            return None;
        }
        let start: usize = skip_space(bytes, index + 1);
        let end: usize = value_end(bytes, start)?;
        found.push((key, (start, end)));
        index = skip_space(bytes, end);
        if bytes.get(index) == Some(&b',') {
            index = skip_space(bytes, index + 1);
        }
    }
    Some(found)
}

/// The element spans of the array spanning `span`.
fn elements(text: &str, span: Span) -> Option<Vec<Span>> {
    let bytes: &[u8] = text.as_bytes();
    if bytes.get(span.0) != Some(&b'[') {
        return None;
    }
    let mut found: Vec<Span> = Vec::new();
    let mut index: usize = skip_space(bytes, span.0 + 1);
    while index < span.1 && bytes[index] != b']' {
        let end: usize = value_end(bytes, index)?;
        found.push((index, end));
        index = skip_space(bytes, end);
        if bytes.get(index) == Some(&b',') {
            index = skip_space(bytes, index + 1);
        }
    }
    Some(found)
}

fn text<'a>(document: &'a str, span: &Span) -> &'a str {
    &document[span.0..span.1]
}

fn whole(text: &str) -> Span {
    let end: usize = text.trim_end().len();
    (skip_space(text.as_bytes(), 0), end)
}

fn member<'a>(found: &'a [(String, Span)], key: &str) -> Option<&'a Span> {
    found
        .iter()
        .find(|(name, _)| name == key)
        .map(|(_, span)| span)
}

/// The string at `path` in the item at `span`, items keeping own keys before nested objects.
fn item_field(text: &str, span: Span, path: &[&str]) -> Option<String> {
    let bytes: &[u8] = text.as_bytes();
    let mut start: usize = span.0;
    for (index, key) in path.iter().enumerate() {
        let needle: String = format!("\"{key}\": ");
        let found: usize = text[start..span.1].find(&needle)?;
        start += found + needle.len();
        if index + 1 < path.len() && bytes.get(start) != Some(&b'{') {
            return None;
        }
    }
    if bytes.get(start) != Some(&b'"') {
        return slow_item_field(text, span, path);
    }
    let end: usize = value_end(bytes, start)?;
    from_json(&text[start..end])
}

fn slow_item_field(text: &str, span: Span, path: &[&str]) -> Option<String> {
    let mut value: Value = from_json(&text[span.0..span.1])?;
    for key in path {
        value = value.get(*key)?.clone();
    }
    value.as_str().map(str::to_owned)
}

/// The stored report patched with the delta's rerun items, diagnostics, counts and timings.
pub(crate) fn spliced_report(
    base: &str,
    delta: &str,
    scope: &SpliceScope<'_>,
) -> Result<String, String> {
    let base_members: Vec<(String, Span)> =
        members(base, whole(base)).ok_or("the stored report cannot be read")?;
    let delta_members: Vec<(String, Span)> =
        members(delta, whole(delta)).ok_or("the delta report cannot be read")?;
    let keys =
        |found: &[(String, Span)]| found.iter().map(|(key, _)| key.clone()).collect::<Vec<_>>();
    if keys(&base_members) != keys(&delta_members) {
        return Err("the reports have different sections".to_owned());
    }
    let mut output: String = String::with_capacity(base.len() + delta.len());
    let mut cursor: usize = 0;
    for ((key, base_span), (_, delta_span)) in base_members.iter().zip(&delta_members) {
        let replacement: String = match key.as_str() {
            "has_errors"
            | "diagnostics"
            | "semantic_checks_partial"
            | "compile_timings"
            | "artifacts" => text(delta, delta_span).to_owned(),
            "summary" => spliced_summary(base, *base_span, delta, *delta_span)
                .ok_or("the summaries do not line up")?,
            "resources" => spliced_resources((base, *base_span), (delta, *delta_span), scope)
                .map_err(|kind| format!("the {kind} do not line up"))?,
            _ => {
                if text(base, base_span) != text(delta, delta_span) {
                    return Err(format!("the reports' {key} differ"));
                }
                continue;
            }
        };
        output.push_str(&base[cursor..base_span.0]);
        output.push_str(&replacement);
        cursor = base_span.1;
    }
    output.push_str(&base[cursor..]);
    Ok(output)
}

fn spliced_summary(base: &str, base_span: Span, delta: &str, delta_span: Span) -> Option<String> {
    let base_members: Vec<(String, Span)> = members(base, base_span)?;
    let delta_members: Vec<(String, Span)> = members(delta, delta_span)?;
    let mut output: String = String::new();
    let mut cursor: usize = base_span.0;
    for (key, span) in &base_members {
        if !DELTA_SUMMARY_KEYS.contains(&key.as_str()) {
            continue;
        }
        let replacement: &Span = member(&delta_members, key)?;
        output.push_str(&base[cursor..span.0]);
        output.push_str(&delta[replacement.0..replacement.1]);
        cursor = span.1;
    }
    output.push_str(&base[cursor..base_span.1]);
    Some(output)
}

fn spliced_resources(
    (base, base_span): (&str, Span),
    (delta, delta_span): (&str, Span),
    scope: &SpliceScope<'_>,
) -> Result<String, String> {
    let resources: String = "resources".to_owned();
    let base_members: Vec<(String, Span)> = members(base, base_span).ok_or(resources.clone())?;
    let delta_members: Vec<(String, Span)> = members(delta, delta_span).ok_or(resources)?;
    let mut output: String = String::new();
    let mut cursor: usize = base_span.0;
    for (key, span) in &base_members {
        let delta_list: &Span = member(&delta_members, key).ok_or(key.clone())?;
        let replacement: Option<Option<String>> = match (key.as_str(), scope.sources) {
            ("sources", Some(sources)) => {
                spliced_items((base, *span), (delta, *delta_list), |document, item| {
                    item_field(document, item, &["name"]).filter(|name| sources.contains(name))
                })
            }
            ("models", _) => {
                spliced_items((base, *span), (delta, *delta_list), |document, item| {
                    item_field(document, item, &["name"]).filter(|name| scope.models.contains(name))
                })
            }
            ("tests", _) => spliced_items((base, *span), (delta, *delta_list), |document, item| {
                item_field(document, item, &["name"]).filter(|name| scope.tests.contains(name))
            }),
            ("audits", _) => {
                spliced_items((base, *span), (delta, *delta_list), |document, item| {
                    let owner: Option<String> =
                        item_field(document, item, &["attached_to", "resource_type"]);
                    let name: Option<String> = item_field(document, item, &["attached_to", "name"]);
                    (owner.as_deref() == Some("model"))
                        .then_some(name)
                        .flatten()
                        .filter(|name| scope.models.contains(name))
                })
            }
            _ => {
                if base[span.0..span.1] != delta[delta_list.0..delta_list.1] {
                    return Err(key.clone());
                }
                Some(None)
            }
        };
        let replacement: Option<String> = replacement.ok_or(key.clone())?;
        if let Some(replacement) = replacement {
            output.push_str(&base[cursor..span.0]);
            output.push_str(&replacement);
            cursor = span.1;
        }
    }
    output.push_str(&base[cursor..base_span.1]);
    Ok(output)
}

/// The base list with owned items replaced by the delta's items of the same owner.
fn spliced_items(
    (base, base_span): (&str, Span),
    (delta, delta_span): (&str, Span),
    owned: impl Fn(&str, Span) -> Option<String>,
) -> Option<Option<String>> {
    let base_items: Vec<Span> = elements(base, base_span)?;
    let delta_items: Vec<Span> = elements(delta, delta_span)?;
    let mut fresh: HashMap<String, std::collections::VecDeque<Span>> = HashMap::new();
    let mut unmatched_delta: std::collections::HashSet<&str> = std::collections::HashSet::new();
    for item in &delta_items {
        match owned(delta, *item) {
            Some(owner) => fresh.entry(owner).or_default().push_back(*item),
            None => {
                unmatched_delta.insert(&delta[item.0..item.1]);
            }
        }
    }
    let mut output: String = String::new();
    let mut cursor: usize = base_span.0;
    let mut replaced: bool = false;
    for item in &base_items {
        let Some(owner) = owned(base, *item) else {
            if !unmatched_delta.is_empty() {
                unmatched_delta.remove(&base[item.0..item.1]);
            }
            continue;
        };
        let replacement: Span = fresh.get_mut(&owner)?.pop_front()?;
        output.push_str(&base[cursor..item.0]);
        output.push_str(&delta[replacement.0..replacement.1]);
        cursor = item.1;
        replaced = true;
    }
    if fresh.values().any(|items| !items.is_empty()) {
        return None;
    }
    if !unmatched_delta.is_empty() {
        return None;
    }
    if !replaced {
        return Some(None);
    }
    output.push_str(&base[cursor..base_span.1]);
    Some(Some(output))
}
