//! Interpolation site scanning and sentinel assembly over byte offsets.

use std::collections::HashSet;
use std::sync::LazyLock;

use regex::Regex;
use sqlbuild_core::text::main::python_isalnum::python_isalnum;
use sqlbuild_core::text::main::python_isspace::python_isspace;
use sqlbuild_sqltext::sql_scan::main::matching_paren::matching_paren;
use sqlbuild_sqltext::sql_scan::main::non_code_end::non_code_end;
use sqlbuild_sqltext::sql_scan::main::quote_policy::quote_policy;
use sqlbuild_sqltext::sql_scan::models::QuotePolicy;

use crate::lint_text::constants::{
    CTE_DEFINITION_PATTERN, FORMAT_SITE_PATTERN, FUNCTION_NAMES, LINT_SITE_PATTERN,
    SENTINEL_SUFFIX, SITE_START_BYTES, TABLE_FUNCTION_NAME, TABLE_FUNCTION_WRAPPER,
};
use crate::lint_text::errors::LintTextError;
use crate::lint_text::main::neutralize_interpolation::neutralize_interpolation;
use crate::lint_text::models::{InterpolationSite, Neutralized};
use crate::lint_text::types::PreparedSql;

pub(crate) static DEPENDENCY_SITE_PATTERN: LazyLock<Result<Regex, regex::Error>> =
    LazyLock::new(|| Regex::new(r"(?i)^__(?:ref|source)\s*\("));
pub(crate) static KEYED_RELATION_SITE_PATTERN: LazyLock<Result<Regex, regex::Error>> =
    LazyLock::new(|| Regex::new(r#"(?i)^__(ref|source|seed)\s*\(\s*['"]([^'"]+)['"]\s*\)$"#));

static CTES: LazyLock<Result<Regex, regex::Error>> =
    LazyLock::new(|| Regex::new(CTE_DEFINITION_PATTERN));
static WORDS: LazyLock<Result<Regex, regex::Error>> =
    LazyLock::new(|| Regex::new(r"\b[A-Za-z_][A-Za-z0-9_]*\b"));
static OPAQUE_CTE: LazyLock<Result<Regex, regex::Error>> =
    LazyLock::new(|| Regex::new(r"(?i)\b[A-Za-z_][A-Za-z0-9_]*\s+AS\s*\(\s*(?:--[^\n]*\n\s*)?$"));
static LINT_SITE: LazyLock<Result<Regex, regex::Error>> =
    LazyLock::new(|| Regex::new(LINT_SITE_PATTERN));
static FORMAT_SITE: LazyLock<Result<Regex, regex::Error>> =
    LazyLock::new(|| Regex::new(FORMAT_SITE_PATTERN));

/// The site-start pattern for lint, or the case-insensitive one for formatting.
pub(crate) fn site_pattern(formatting: bool) -> Result<&'static Regex, LintTextError> {
    let pattern: &'static LazyLock<Result<Regex, regex::Error>> =
        if formatting { &FORMAT_SITE } else { &LINT_SITE };
    compiled(pattern)
}

/// A lazily compiled built-in pattern, or its compile error.
pub(crate) fn compiled(
    pattern: &'static LazyLock<Result<Regex, regex::Error>>,
) -> Result<&'static Regex, LintTextError> {
    pattern
        .as_ref()
        .map_err(|error| LintTextError::Pattern(error.to_string()))
}

/// One replaced byte range and the sentinel that replaces it.
pub(crate) type ByteSite = (usize, usize, String);

/// The lexical policy lint and format scan authored SQL with.
pub(crate) fn lint_policy(backtick_identifiers: bool) -> QuotePolicy {
    QuotePolicy::SQL_LINT.with_backtick_identifiers(backtick_identifiers)
}

/// Offsets of candidate sites outside comments and quoted text, as Python's `finditer` yields them.
pub(crate) fn site_starts(text: &str, pattern: &Regex, policy: QuotePolicy) -> Vec<usize> {
    let bytes: &[u8] = text.as_bytes();
    let mut starts: Vec<usize> = Vec::new();
    let mut index: usize = 0;
    while index < bytes.len() {
        match non_code_end(bytes, index, policy) {
            Ok(Some(end)) => {
                index = end;
                continue;
            }
            Err(_) => break,
            Ok(None) => {}
        }
        let found = SITE_START_BYTES
            .contains(&bytes[index])
            .then(|| text.get(index..).and_then(|rest| pattern.find(rest)))
            .flatten();
        if let Some(found) = found {
            starts.push(index);
            index += found.end();
        } else {
            index += 1;
        }
    }
    starts
}

/// The end of the interpolation site starting at `start`, if one starts there.
pub(crate) fn site_end(text: &str, start: usize, policy: QuotePolicy) -> Option<usize> {
    let bytes: &[u8] = text.as_bytes();
    match *bytes.get(start)? {
        b'@' if bytes.get(start + 1) == Some(&b'@') => Some(name_end(text, start + 2, true)),
        b'@' => macro_site_end(text, start, policy),
        b'$' => bytes
            .get(start + 2..)?
            .iter()
            .position(|byte| *byte == b'}')
            .map(|index| start + 2 + index + 1),
        b'_' => {
            let end: usize = name_end(text, start, false);
            let name: String = text[start..end].to_lowercase();
            if !FUNCTION_NAMES.contains(&name.as_str()) {
                return None;
            }
            let open: usize = skip_whitespace(text, end);
            (bytes.get(open) == Some(&b'('))
                .then(|| paren_end(bytes, open, policy))
                .flatten()
        }
        _ => None,
    }
}

fn macro_site_end(text: &str, start: usize, policy: QuotePolicy) -> Option<usize> {
    let name_start: usize = start + 1;
    let first: char = text.get(name_start..)?.chars().next()?;
    if first == '\'' {
        return text[name_start + 1..]
            .find('\'')
            .map(|index| name_start + 1 + index + 1);
    }
    if !(first.is_alphabetic() || first == '_') {
        return None;
    }
    let mut end: usize = name_end(text, name_start, false);
    if matches!(&text[name_start..end], "py" | "sql")
        && text.as_bytes().get(end) == Some(&b'.')
        && text[end + 1..]
            .chars()
            .next()
            .is_some_and(|next| next.is_alphabetic() || next == '_')
    {
        end = name_end(text, end + 1, false);
    }
    if text.as_bytes().get(end) == Some(&b'(') {
        paren_end(text.as_bytes(), end, policy)
    } else {
        Some(end)
    }
}

/// The end of a name: alphanumerics and `_`, plus `:` and `.` inside `@@` interpolation.
pub(crate) fn name_end(text: &str, start: usize, interpolation: bool) -> usize {
    let mut end: usize = start;
    for character in text[start..].chars() {
        if python_isalnum(character)
            || character == '_'
            || interpolation && matches!(character, ':' | '.')
        {
            end += character.len_utf8();
        } else {
            break;
        }
    }
    end
}

fn skip_whitespace(text: &str, start: usize) -> usize {
    let mut end: usize = start;
    for character in text[start..].chars() {
        if !python_isspace(character) {
            break;
        }
        end += character.len_utf8();
    }
    end
}

fn paren_end(bytes: &[u8], open: usize, policy: QuotePolicy) -> Option<usize> {
    match matching_paren(bytes, open, policy) {
        Ok(close) => Some(close + 1),
        Err(_) => None,
    }
}

/// The end of the call a `__table_fn(...)` site at `start..end` opens, unless `TABLE( )` wraps it.
pub(crate) fn unwrapped_table_function_end(
    text: &str,
    (start, end): (usize, usize),
    policy: QuotePolicy,
) -> Option<usize> {
    let name_end: usize = name_end(text, start, false);
    if !text[start..name_end].eq_ignore_ascii_case(TABLE_FUNCTION_NAME) {
        return None;
    }
    let open: usize = skip_whitespace(text, end);
    if text.as_bytes().get(open) != Some(&b'(') || inside_table_wrapper(&text[..start]) {
        return None;
    }
    paren_end(text.as_bytes(), open, policy)
}

/// Whether `before` ends with `TABLE (`, the wrapper a table function may already be written in.
fn inside_table_wrapper(before: &str) -> bool {
    let keyword: &str = TABLE_FUNCTION_WRAPPER.trim_end_matches('(');
    let Some(before) = before.trim_end().strip_suffix('(') else {
        return false;
    };
    let before: &str = before.trim_end();
    before.len() >= keyword.len()
        && before.is_char_boundary(before.len() - keyword.len())
        && before[before.len() - keyword.len()..].eq_ignore_ascii_case(keyword)
        && !before[..before.len() - keyword.len()]
            .chars()
            .next_back()
            .is_some_and(|character| python_isalnum(character) || character == '_')
}

/// A numbered sentinel such as `__sqlclean_interpolation_0__`.
pub(crate) fn sentinel(prefix: &str, index: usize) -> String {
    format!("{prefix}{index}{SENTINEL_SUFFIX}")
}

/// Replace ascending, non-overlapping byte ranges by their sentinels, with character offsets.
pub(crate) fn assemble(body: &str, byte_sites: Vec<ByteSite>) -> Neutralized {
    let mut text: String = String::with_capacity(body.len());
    let mut sites: Vec<InterpolationSite> = Vec::with_capacity(byte_sites.len());
    let mut copied_to: usize = 0;
    let mut original_chars: usize = 0;
    let mut output_chars: usize = 0;
    for (start, end, sentinel) in byte_sites {
        let literal: &str = &body[copied_to..start];
        let literal_chars: usize = literal.chars().count();
        text.push_str(literal);
        original_chars += literal_chars;
        output_chars += literal_chars;
        let original: &str = &body[start..end];
        let original_len: usize = original.chars().count();
        let sentinel_len: usize = sentinel.chars().count();
        text.push_str(&sentinel);
        sites.push(InterpolationSite {
            sentinel,
            neutralized_start: output_chars,
            neutralized_end: output_chars + sentinel_len,
            original_start: original_chars,
            original_end: original_chars + original_len,
            original_text: original.to_string(),
        });
        original_chars += original_len;
        output_chars += sentinel_len;
        copied_to = end;
    }
    text.push_str(&body[copied_to..]);
    Neutralized { text, sites }
}

/// The byte offset of character position `position`, or `None` past the end.
pub(crate) fn byte_offset(text: &str, position: usize) -> Option<usize> {
    if text.is_ascii() {
        return (position <= text.len()).then_some(position);
    }
    text.char_indices()
        .map(|(index, _)| index)
        .chain(std::iter::once(text.len()))
        .nth(position)
}

/// End of an ASCII `[A-Za-z_][A-Za-z0-9_]*` name at `start`, or `start` when none begins there.
pub(crate) fn ascii_name_end(bytes: &[u8], start: usize) -> usize {
    match bytes.get(start) {
        Some(byte) if byte.is_ascii_alphabetic() || *byte == b'_' => {}
        _ => return start,
    }
    let mut end: usize = start + 1;
    while bytes
        .get(end)
        .is_some_and(|byte| byte.is_ascii_alphanumeric() || *byte == b'_')
    {
        end += 1;
    }
    end
}

/// Whether `@py.name(` or `@sql.name(` starts at the kind name ending at `end`.
pub(crate) fn opens_kind_call(body: &str, start: usize, end: usize) -> bool {
    let bytes: &[u8] = body.as_bytes();
    if !matches!(&body[start..end], "py" | "sql") || bytes.get(end) != Some(&b'.') {
        return false;
    }
    let name_end: usize = ascii_name_end(bytes, end + 1);
    name_end > end + 1 && opens_call(body, name_end)
}

pub(crate) fn opens_call(body: &str, end: usize) -> bool {
    body[end..]
        .chars()
        .find(|character| !python_isspace(*character))
        == Some('(')
}

/// Return whether SQL lint treats backticks as identifier quotes for `dialect`, as rules do.
pub(crate) fn backtick_identifiers(dialect: &str) -> bool {
    quote_policy(dialect).backtick_identifiers
}

/// The neutralized SQL, its sites and the CTEs interpolation references, or `None` when a prior site is out of range.
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
