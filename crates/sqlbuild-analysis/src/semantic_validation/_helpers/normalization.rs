//! Analysis normalization with a character-offset provenance map.

use crate::semantic_checks::constants::UDF_NAME_PREFIX;
use regex::Regex;
use sqlbuild_sqltext::sql_scan::main::matching_paren::matching_paren;
use sqlbuild_sqltext::sql_scan::main::non_code_end::non_code_end;
use sqlbuild_sqltext::sql_scan::models::QuotePolicy;
use std::collections::HashMap;
use std::sync::LazyLock;

const DBT_REF: &str = "dbt_ref";
const UDF: &str = "udf";
/// A hook parameter `@'name'` becomes a string literal with this prefix, as SQLBuild fills it.
const HOOK_PARAMETER_PREFIX: &str = "__sqlbuild_parameter__";
/// Snowflake's second line comment prefix; only Snowflake SQL reaches the variant scan.
const SNOWFLAKE_LINE_COMMENT: &str = "//";
type Edit = (usize, usize, MappedSql);
static REFERENCES: LazyLock<Result<Regex, String>> = LazyLock::new(|| {
    Regex::new(r#"__(ref|seed|source|dbt_ref)\((?:"[^"]+"\s*,\s*)?"([^"]+)"\)"#)
        .map_err(|error| error.to_string())
});
static FUNCTIONS: LazyLock<Result<Regex, String>> = LazyLock::new(|| {
    Regex::new(r#"__(udf|table_fn)\("([A-Za-z_][A-Za-z0-9_]*)"\)\s*"#)
        .map_err(|error| error.to_string())
});
static NATIVE_STUBS: LazyLock<Result<Regex, String>> = LazyLock::new(|| {
    Regex::new(r"__cursor_(start|end)\(\s*\)|@'([A-Za-z_][A-Za-z0-9_]*)'")
        .map_err(|error| error.to_string())
});
static PLACEHOLDERS: LazyLock<Result<Regex, String>> =
    LazyLock::new(|| Regex::new(r"@@@(\w+)").map_err(|error| error.to_string()));
static VARIANT_BASE: LazyLock<Result<Regex, String>> = LazyLock::new(|| {
    Regex::new(r#"(?:[A-Za-z_][A-Za-z0-9_$]*|"(?:""|[^"])+")(?:\.(?:[A-Za-z_][A-Za-z0-9_$]*|"(?:""|[^"])+"))*:(?:[A-Za-z_][A-Za-z0-9_$]*|"(?:""|[^"])+")(?:(?::|\.)(?:[A-Za-z_][A-Za-z0-9_$]*|"(?:""|[^"])+"))*$"#).map_err(|error| error.to_string())
});
static SIMPLE_KEY: LazyLock<Result<Regex, String>> = LazyLock::new(|| {
    Regex::new(r#"^\s*(?:[A-Za-z_][A-Za-z0-9_$]*|"(?:""|[^"])+"|[0-9]+|'(?:''|[^'])*')\s*$"#)
        .map_err(|error| error.to_string())
});

#[derive(Clone)]
pub(super) struct MappedSql {
    pub(super) sql: String,
    pub(super) offsets: Vec<usize>,
}

impl MappedSql {
    pub(super) fn new(sql: &str) -> Self {
        Self {
            sql: sql.to_owned(),
            offsets: (0..=sql.chars().count()).collect(),
        }
    }

    fn slice(&self, start: usize, end: usize) -> Self {
        let first = self.sql[..start].chars().count();
        let last = first + self.sql[start..end].chars().count();
        Self {
            sql: self.sql[start..end].to_owned(),
            offsets: self.offsets[first..=last].to_vec(),
        }
    }

    fn generated(text: String, anchor: usize) -> Self {
        Self {
            offsets: vec![anchor; text.chars().count() + 1],
            sql: text,
        }
    }

    fn append(&mut self, other: Self) {
        self.sql.push_str(&other.sql);
        self.offsets.pop();
        self.offsets.extend(other.offsets);
    }

    fn edits(self, edits: Vec<(usize, usize, Self)>) -> Self {
        if edits.is_empty() {
            return self;
        }
        let mut result = self.slice(0, 0);
        let mut previous = 0;
        for (start, end, replacement) in edits {
            result.append(self.slice(previous, start));
            result.append(replacement);
            previous = end;
        }
        result.append(self.slice(previous, self.sql.len()));
        result
    }
}

pub(super) fn normalize(
    sql: &str,
    dialect: &str,
    (stubs, placeholders): (&HashMap<String, String>, &HashMap<String, String>),
    udf_prefix: &str,
) -> Result<MappedSql, String> {
    let mut result = MappedSql::new(sql);
    let mut edits: Vec<Edit> = Vec::new();
    for captures in REFERENCES
        .as_ref()
        .map_err(Clone::clone)?
        .captures_iter(&result.sql)
    {
        let whole = captures.get(0).ok_or("missing reference match")?;
        let name = captures.get(2).ok_or("missing reference name")?;
        let replacement = if &captures[1] != DBT_REF && stubs.contains_key(name.as_str()) {
            MappedSql::generated(
                stubs[name.as_str()].clone(),
                result.offsets[result.sql[..name.start()].chars().count()],
            )
        } else {
            result.slice(name.start(), name.end())
        };
        edits.push((whole.start(), whole.end(), replacement));
    }
    result = result.edits(edits);
    let mut edits: Vec<Edit> = Vec::new();
    let mut consumed_until = 0;
    for captures in FUNCTIONS
        .as_ref()
        .map_err(Clone::clone)?
        .captures_iter(&result.sql)
    {
        let whole = captures.get(0).ok_or("missing function match")?;
        if whole.start() < consumed_until {
            continue;
        }
        if result.sql.as_bytes().get(whole.end()) != Some(&b'(') {
            continue;
        }
        let name = captures.get(2).ok_or("missing function name")?;
        let mut end = whole.end();
        let stub = if &captures[1] == UDF {
            format!("{udf_prefix}{}", name.as_str())
        } else {
            end = matching_paren(result.sql.as_bytes(), whole.end(), QuotePolicy::COMPILER)
                .map_err(|error| format!("invalid table-function analysis input: {error:?}"))?
                + 1;
            let default = format!("__sqlbuild_table_function_{}", name.as_str());
            stubs.get(&default).cloned().unwrap_or(default)
        };
        edits.push((
            whole.start(),
            end,
            MappedSql::generated(
                stub,
                result.offsets[result.sql[..whole.start()].chars().count()],
            ),
        ));
        consumed_until = end;
    }
    result = result.edits(edits);
    if dialect.eq_ignore_ascii_case("snowflake") {
        result = normalize_variant(result)?;
    }
    let mut edits: Vec<Edit> = Vec::new();
    for captures in PLACEHOLDERS
        .as_ref()
        .map_err(Clone::clone)?
        .captures_iter(&result.sql)
    {
        if let Some(value) = placeholders.get(&captures[1]) {
            let whole = captures.get(0).ok_or("missing placeholder match")?;
            edits.push((
                whole.start(),
                whole.end(),
                MappedSql::generated(
                    value.clone(),
                    result.offsets[result.sql[..whole.start()].chars().count()],
                ),
            ));
        }
    }
    Ok(result.edits(edits))
}

pub(crate) fn normalize_analysis_sql(
    sql: &str,
    dialect: &str,
    stubs: HashMap<String, String>,
    placeholders: HashMap<String, String>,
) -> Result<String, String> {
    normalize(sql, dialect, (&stubs, &placeholders), UDF_NAME_PREFIX).map(|mapped| mapped.sql)
}

/// Analysis SQL as a native parser reads it, with every stub spelled under `stub_prefix`.
pub(crate) fn normalize_native_sql(
    sql: &str,
    dialect: &str,
    stub_prefix: &str,
) -> Result<String, String> {
    let none: HashMap<String, String> = HashMap::new();
    let udf_prefix: String = format!("{stub_prefix}udf_");
    let normalized: String = normalize(sql, dialect, (&none, &none), &udf_prefix)?.sql;
    let stubs: &Regex = NATIVE_STUBS.as_ref()?;
    let non_code: Vec<(usize, usize)> = non_code_ranges(&normalized)?;
    let mut native: String = String::with_capacity(normalized.len());
    let mut copied_until: usize = 0;
    for captures in stubs.captures_iter(&normalized) {
        let whole = captures.get(0).ok_or("missing stub match")?;
        let quoted: bool = non_code
            .iter()
            .any(|&(start, end)| start <= whole.start() && whole.start() < end);
        let glued: bool = captures.get(1).is_some()
            && normalized[..whole.start()]
                .chars()
                .next_back()
                .is_some_and(|character| character.is_alphanumeric() || character == '_');
        if quoted || glued {
            continue;
        }
        native.push_str(&normalized[copied_until..whole.start()]);
        match (captures.get(1), captures.get(2)) {
            (Some(bound), _) => {
                native.push_str(&format!("{stub_prefix}cursor_{}()", bound.as_str()));
            }
            (None, Some(parameter)) => {
                native.push_str(&format!("'{HOOK_PARAMETER_PREFIX}{}'", parameter.as_str()));
            }
            (None, None) => native.push_str(whole.as_str()),
        }
        copied_until = whole.end();
    }
    native.push_str(&normalized[copied_until..]);
    Ok(native)
}

/// The byte ranges of `sql`'s strings, quoted identifiers and comments, in order.
fn non_code_ranges(sql: &str) -> Result<Vec<(usize, usize)>, String> {
    let mut ranges: Vec<(usize, usize)> = Vec::new();
    let mut index: usize = 0;
    while index < sql.len() {
        if let Some(end) = skip_non_code(sql, index)? {
            ranges.push((index, end));
            index = end;
            continue;
        }
        index += sql[index..].chars().next().map_or(1, char::len_utf8);
    }
    Ok(ranges)
}

pub(crate) fn normalize_dialect_sql_mapped(
    sql: &str,
    dialect: &str,
) -> Result<(String, Vec<usize>), String> {
    let input = MappedSql::new(sql);
    if !dialect.eq_ignore_ascii_case("snowflake") {
        return Ok((input.sql, input.offsets));
    }
    normalize_variant(input).map(|mapped| (mapped.sql, mapped.offsets))
}

pub(crate) fn normalize_dialect_sql(sql: &str, dialect: &str) -> Result<String, String> {
    if !dialect.eq_ignore_ascii_case("snowflake") {
        return Ok(sql.to_owned());
    }
    normalize_variant(MappedSql::new(sql)).map(|mapped| mapped.sql)
}

fn skip_non_code(sql: &str, index: usize) -> Result<Option<usize>, String> {
    if sql[index..].starts_with("$$") {
        return Ok(Some(
            sql[index + 2..]
                .find("$$")
                .map_or(sql.len(), |end| index + 4 + end),
        ));
    }
    if sql[index..].starts_with(SNOWFLAKE_LINE_COMMENT) {
        return Ok(Some(
            sql[index..]
                .find('\n')
                .map_or(sql.len(), |offset| index + offset + 1),
        ));
    }
    non_code_end(
        sql.as_bytes(),
        index,
        QuotePolicy {
            backtick_identifiers: true,
            single_quote_backslash_escapes: true,
            double_quote_backslash_escapes: false,
            dollar_quotes: true,
        },
    )
    .map_err(|error| format!("invalid SQL analysis input: {error:?}"))
}

fn matching_bracket(sql: &str, start: usize) -> Result<Option<usize>, String> {
    let mut depth = 1;
    let mut index = start + 1;
    while index < sql.len() {
        if let Some(end) = skip_non_code(sql, index)? {
            index = end;
            continue;
        }
        match sql.as_bytes()[index] {
            b'[' => depth += 1,
            b']' => {
                depth -= 1;
                if depth == 0 {
                    return Ok(Some(index));
                }
            }
            _ => {}
        }
        index += sql[index..].chars().next().map_or(1, char::len_utf8);
    }
    Ok(None)
}

fn normalize_variant(input: MappedSql) -> Result<MappedSql, String> {
    let sql = &input.sql;
    let mut edits: Vec<Edit> = Vec::new();
    let mut index = 0;
    while index < sql.len() {
        if let Some(end) = skip_non_code(sql, index)? {
            index = end;
            continue;
        }
        if sql.as_bytes()[index] != b'[' {
            index += sql[index..].chars().next().map_or(1, char::len_utf8);
            continue;
        }
        let Some(close) = matching_bracket(sql, index)? else {
            index += 1;
            continue;
        };
        if SIMPLE_KEY
            .as_ref()
            .map_err(Clone::clone)?
            .is_match(&sql[index + 1..close])
        {
            index = close + 1;
            continue;
        }
        let key = normalize_variant(input.slice(index + 1, close))?;
        let mut base_end = index;
        loop {
            base_end = sql[..base_end].trim_end().len();
            if !sql[..base_end].ends_with("*/") {
                break;
            }
            let Some(open) = sql[..base_end - 2].rfind("/*") else {
                break;
            };
            base_end = open;
        }
        if let Some(base) = VARIANT_BASE
            .as_ref()
            .map_err(Clone::clone)?
            .find(&sql[..base_end])
        {
            let anchor = input.offsets[sql[..base.start()].chars().count()];
            let mut replacement = MappedSql::generated("GET(".to_owned(), anchor);
            replacement.append(input.slice(base.start(), index));
            replacement.append(MappedSql::generated(
                ", ".to_owned(),
                input.offsets[sql[..index].chars().count()],
            ));
            replacement.append(key);
            replacement.append(MappedSql::generated(
                ")".to_owned(),
                input.offsets[sql[..close].chars().count()],
            ));
            edits.push((base.start(), close + 1, replacement));
        } else if key.sql != sql[index + 1..close] {
            edits.push((index + 1, close, key));
        }
        index = close + 1;
    }
    Ok(input.edits(edits))
}
