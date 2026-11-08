//! `@name` parameter references in a SQL macro body, as generic audits write their parameters.

use sqlbuild_render::macro_calls::main::opens_bare_call::opens_bare_call;

use crate::sql_macros::_helpers::parsing::lexing::{
    DECLARATION_WORDS, Token, TokenKind, closing_token, tokens,
};

const PLACEHOLDER_PREFIX: &str = "__sqlbuild_parameter__";
/// Words that may follow a table reference without being its alias.
const FOLLOWING_KEYWORDS: [&str; 34] = [
    "WHERE",
    "JOIN",
    "LEFT",
    "RIGHT",
    "INNER",
    "FULL",
    "CROSS",
    "OUTER",
    "NATURAL",
    "ON",
    "USING",
    "GROUP",
    "ORDER",
    "LIMIT",
    "OFFSET",
    "FETCH",
    "HAVING",
    "QUALIFY",
    "WINDOW",
    "UNION",
    "EXCEPT",
    "INTERSECT",
    "MINUS",
    "LATERAL",
    "PIVOT",
    "UNPIVOT",
    "SAMPLE",
    "TABLESAMPLE",
    "ASOF",
    "POSITIONAL",
    "ANTI",
    "SEMI",
    "FOR",
    "AT",
];

/// One `@name` parameter reference: byte spans of the whole reference and of the name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ParameterReference {
    pub(crate) start: usize,
    pub(crate) end: usize,
    pub(crate) name: String,
    /// Index of the reference's name token.
    pub(crate) token: usize,
}

/// Every `@name` of `sql` that is neither a macro call, a built-in call nor `@@`.
pub(crate) fn parameter_references(sql: &str, scanned: &[Token]) -> Vec<ParameterReference> {
    let mut found: Vec<ParameterReference> = Vec::new();
    for (index, at) in scanned.iter().enumerate() {
        if !at.is(sql, "@") {
            continue;
        }
        if index > 0 && scanned[index - 1].is(sql, "@") && scanned[index - 1].end == at.start {
            continue;
        }
        let Some(word) = scanned
            .get(index + 1)
            .filter(|word| word.kind == TokenKind::Word && word.start == at.end)
        else {
            continue;
        };
        let after: Option<&Token> = scanned.get(index + 2);
        let name: &str = word.text(sql);
        let declaration: bool =
            DECLARATION_WORDS.contains(&name) && after.is_some_and(|open| open.is(sql, "("));
        if declaration || opens_bare_call(sql, word.end) {
            continue;
        }
        if matches!(name, "sql" | "py")
            && after.is_some_and(|dot| dot.is(sql, ".") && dot.start == word.end)
            && scanned.get(index + 4).is_some_and(|open| open.is(sql, "("))
        {
            continue;
        }
        found.push(ParameterReference {
            start: at.start,
            end: word.end,
            name: name.to_owned(),
            token: index + 1,
        });
    }
    found
}

/// The word placed where `@name` stood while the body is parsed for its shape.
pub(crate) fn placeholder(name: &str) -> String {
    format!("{PLACEHOLDER_PREFIX}{name}")
}

/// The parameter a placeholder word names, if the word is one.
pub(crate) fn placeholder_parameter(word: &str) -> Option<&str> {
    word.strip_prefix(PLACEHOLDER_PREFIX)
}

/// `sql` with each reference replaced by its placeholder word, for parsing.
pub(crate) fn with_placeholders(sql: &str, references: &[ParameterReference]) -> String {
    let mut text: String =
        String::with_capacity(sql.len() + references.len() * PLACEHOLDER_PREFIX.len());
    let mut cursor: usize = 0;
    for reference in references {
        text.push_str(&sql[cursor..reference.start]);
        text.push_str(&placeholder(&reference.name));
        cursor = reference.end;
    }
    text.push_str(&sql[cursor..]);
    text
}

/// `sql` with each `@enum(...)[.NAME]`, `@const(...)` and `@param(...)` as a name of the same length.
pub(crate) fn declarations_masked(sql: &str) -> String {
    let Ok(scanned) = tokens(sql) else {
        return sql.to_owned();
    };
    let mut masked: String = sql.to_owned();
    let mut index: usize = 0;
    while index + 2 < scanned.len() {
        let (at, word, open) = (&scanned[index], &scanned[index + 1], &scanned[index + 2]);
        let declaration: bool = at.is(sql, "@")
            && word.kind == TokenKind::Word
            && word.start == at.end
            && DECLARATION_WORDS.contains(&word.text(sql))
            && open.is(sql, "(");
        if !declaration {
            index += 1;
            continue;
        }
        let mut depth: usize = 0;
        let mut close: usize = index + 2;
        for (offset, token) in scanned[index + 2..].iter().enumerate() {
            depth = match (token.is(sql, "("), token.is(sql, ")")) {
                (true, _) => depth + 1,
                (_, true) => depth - 1,
                _ => depth,
            };
            if depth == 0 {
                close = index + 2 + offset;
                break;
            }
        }
        let member: bool = scanned.get(close + 1).is_some_and(|dot| dot.is(sql, "."))
            && scanned
                .get(close + 2)
                .is_some_and(|name| name.kind == TokenKind::Word);
        let last: usize = if member { close + 2 } else { close };
        let (start, end) = (at.start, scanned[last].end);
        masked.replace_range(start..end, &format!("d{}", "_".repeat(end - start - 1)));
        index = last + 1;
    }
    masked
}

/// `sql` with each placeholder slot read in a `USING` list as one word of the same length, since
/// `USING` names bare columns: `__sqlbuild_parameter__r.keys` reads as `__sqlbuild_parameter__r_keys`.
pub(crate) fn using_reads_joined(sql: &str) -> String {
    let Ok(scanned) = tokens(sql) else {
        return sql.to_owned();
    };
    let mut joined: String = sql.to_owned();
    let mut index: usize = 0;
    while index + 1 < scanned.len() {
        if !(scanned[index].is_word(sql, "USING") && scanned[index + 1].is(sql, "(")) {
            index += 1;
            continue;
        }
        let close: usize = closing_token(sql, &scanned, index + 1).unwrap_or(scanned.len());
        for read in scanned[index + 2..close].windows(3) {
            let (relation, dot, slot) = (&read[0], &read[1], &read[2]);
            if relation.kind == TokenKind::Word
                && placeholder_parameter(relation.text(sql)).is_some()
                && dot.is(sql, ".")
                && dot.start == relation.end
                && slot.kind == TokenKind::Word
                && slot.start == dot.end
            {
                joined.replace_range(dot.start..dot.end, "_");
            }
        }
        index = close;
    }
    joined
}

/// Whether the table reference whose last token is `index` is followed by an alias.
pub(crate) fn alias_follows(sql: &str, scanned: &[Token], index: usize) -> bool {
    match scanned.get(index + 1) {
        Some(next) if next.is_word(sql, "AS") => true,
        Some(next) if next.kind == TokenKind::Word => !FOLLOWING_KEYWORDS
            .iter()
            .any(|keyword| next.is_word(sql, keyword)),
        Some(next) => next.kind == TokenKind::Quoted,
        None => false,
    }
}
