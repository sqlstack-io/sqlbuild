//! The canonical token stream of SQL under SQLQuack's tokenizers.

use sqlclean::sql_tokens::main::is_unquoted_word::is_unquoted_word;
use sqlclean::sql_tokens::main::name_positions::name_positions;
use sqlquack_syntax::lossless::models::TokenKind;
use sqlquack_syntax_postgres::scan::main::scan::scan;
use sqlquack_syntax_postgres::scan::models::TokenKind as PostgresTokenKind;

use crate::sql_tokens::_helpers::reserved_keywords::is_reserved_keyword;
use crate::sql_tokens::constants::{
    CALL_CLOSER, CALL_OPENER, COLUMN_LIST_RELATION_KEYWORDS, CTE_BODY_OPENERS, CTE_BODY_PREFIX,
    PLACEHOLDER_PREFIX,
};
use crate::sql_tokens::models::{FingerprintTokenizer, SqlquackFingerprintDialect};

/// Significant token texts with reserved keywords and called names, except `__` placeholders, upper-cased.
pub(crate) fn canonical_tokens(
    sql: &str,
    dialect: SqlquackFingerprintDialect,
) -> Result<Vec<String>, String> {
    let raws: Vec<&str> = significant_texts(sql, dialect.tokenizer)?;
    let uppers: Vec<String> = raws.iter().map(|raw| raw.to_ascii_uppercase()).collect();
    let names: Vec<bool> = name_positions(&uppers);
    let closers: Vec<Option<usize>> = matching_closers(&uppers);
    let mut texts: Vec<String> = Vec::with_capacity(raws.len());
    for (index, raw) in raws.iter().enumerate() {
        let folds = !names[index]
            && is_unquoted_word(raw)
            && (is_reserved_keyword(dialect.keywords, &uppers[index])
                || calls(raw, &uppers, &closers, index));
        texts.push(if folds {
            uppers[index].clone()
        } else {
            (*raw).to_owned()
        });
    }
    Ok(texts)
}

/// The text of each significant token of `sql`, in order.
fn significant_texts(sql: &str, tokenizer: FingerprintTokenizer) -> Result<Vec<&str>, String> {
    match tokenizer {
        FingerprintTokenizer::Lossless(dialect) => Ok(sqlquack_syntax::tokenize(sql, dialect)
            .map_err(|error| format!("SQL tokenization failed: {error}"))?
            .iter()
            .filter(|token| matches!(token.kind, TokenKind::Significant(_)))
            .map(|token| token.text(sql))
            .collect()),
        FingerprintTokenizer::PostgresScanner => scan(sql)
            .into_iter()
            .map(|token| match token.kind {
                PostgresTokenKind::Fault(_) => {
                    Err(format!("SQL tokenization failed: {}", token.value))
                }
                _ => sql
                    .get(token.start..token.end)
                    .ok_or_else(|| format!("SQL tokenization failed at byte {}", token.start)),
            })
            .collect(),
    }
}

/// The index of the `)` closing each `(`, by the index of the `(`.
fn matching_closers(uppers: &[String]) -> Vec<Option<usize>> {
    let mut closers: Vec<Option<usize>> = vec![None; uppers.len()];
    let mut open: Vec<usize> = Vec::new();
    for (index, token) in uppers.iter().enumerate() {
        if token == CALL_OPENER {
            open.push(index);
        } else if token == CALL_CLOSER
            && let Some(opener) = open.pop()
        {
            closers[opener] = Some(index);
        }
    }
    closers
}

/// Whether the word at `index` is called: followed by `(` and neither a placeholder nor a relation.
fn calls(raw: &str, uppers: &[String], closers: &[Option<usize>], index: usize) -> bool {
    if uppers.get(index + 1).is_none_or(|next| next != CALL_OPENER)
        || raw.starts_with(PLACEHOLDER_PREFIX)
    {
        return false;
    }
    let previous: Option<&str> = index.checked_sub(1).map(|before| uppers[before].as_str());
    if previous.is_some_and(|word| COLUMN_LIST_RELATION_KEYWORDS.contains(&word)) {
        return false;
    }
    let Some(closer) = closers[index + 1] else {
        return true;
    };
    !(uppers
        .get(closer + 1)
        .is_some_and(|next| next == CTE_BODY_PREFIX)
        && uppers
            .get(closer + 2)
            .is_some_and(|next| CTE_BODY_OPENERS.contains(&next.as_str())))
}
