//! One native lexical pass for interpolation provenance and CTE dependency evidence.

use std::collections::HashSet;
use std::sync::LazyLock;

use regex::Regex;

use crate::lint_text::main::neutralize_interpolation::neutralize_interpolation;
use crate::lint_text::models::Neutralized;
use crate::lint_text::types::PreparedSql;
use sqlbuild_sqltext::sql_scan::main::quote_policy::quote_policy;

static CTES: LazyLock<Result<Regex, regex::Error>> = LazyLock::new(|| {
    Regex::new(
        r#"(?is)(?:\bWITH(?:\s+RECURSIVE)?|,)\s*["`\[]?(?P<name>[A-Za-z_][A-Za-z0-9_]*)["`\]]?(?:\s*\([^)]*\))?\s+AS\s*\("#,
    )
});
static WORDS: LazyLock<Result<Regex, regex::Error>> =
    LazyLock::new(|| Regex::new(r"\b[A-Za-z_][A-Za-z0-9_]*\b"));
static OPAQUE_CTE: LazyLock<Result<Regex, regex::Error>> =
    LazyLock::new(|| Regex::new(r"(?i)\b[A-Za-z_][A-Za-z0-9_]*\s+AS\s*\(\s*(?:--[^\n]*\n\s*)?$"));

/// Return whether SQL lint treats backticks as identifier quotes for `dialect`, as rules do.
pub(crate) fn backtick_identifiers(dialect: &str) -> bool {
    quote_policy(dialect).backtick_identifiers
}

pub(crate) fn prepare(
    expanded: &str,
    before_expansion: &str,
    prior_sites: &[usize],
    dialect: &str,
) -> Result<Option<PreparedSql>, String> {
    let cte_pattern = CTES.as_ref().map_err(std::string::ToString::to_string)?;
    let word_pattern = WORDS.as_ref().map_err(std::string::ToString::to_string)?;
    let opaque_pattern = OPAQUE_CTE
        .as_ref()
        .map_err(std::string::ToString::to_string)?;
    let neutralized: Neutralized =
        neutralize_interpolation(expanded, dialect, false).map_err(|error| error.to_string())?;
    let names: HashSet<String> = cte_pattern
        .captures_iter(expanded)
        .map(|capture| capture["name"].to_ascii_lowercase())
        .collect();
    let mut referenced: HashSet<String> = HashSet::new();
    for site in &neutralized.sites {
        referenced.extend(
            word_pattern
                .find_iter(&site.original_text)
                .map(|word| word.as_str().to_ascii_lowercase())
                .filter(|word| names.contains(word)),
        );
    }
    for &start in prior_sites {
        let Some(prefix) = byte_offset(before_expansion, start)
            .and_then(|byte_start| before_expansion.get(..byte_start))
        else {
            return Ok(None);
        };
        if opaque_pattern.is_match(prefix) {
            referenced.extend(
                cte_pattern
                    .captures_iter(prefix)
                    .map(|capture| capture["name"].to_ascii_lowercase())
                    .filter(|name| names.contains(name)),
            );
        }
    }
    let mut referenced: Vec<String> = referenced.into_iter().collect();
    referenced.sort();
    Ok(Some((neutralized.text, neutralized.sites, referenced)))
}

/// Every CTE name `sql` defines, lowercased, in order and without repeats.
pub(crate) fn defined_cte_names(sql: &str) -> Vec<String> {
    let Ok(cte_pattern) = CTES.as_ref() else {
        return Vec::new();
    };
    let mut names: Vec<String> = Vec::new();
    for capture in cte_pattern.captures_iter(sql) {
        let name: String = capture["name"].to_ascii_lowercase();
        if !names.contains(&name) {
            names.push(name);
        }
    }
    names
}

/// The byte offset of character position `position`, or `None` past the end.
fn byte_offset(text: &str, position: usize) -> Option<usize> {
    text.char_indices()
        .map(|(index, _)| index)
        .chain(std::iter::once(text.len()))
        .nth(position)
}
