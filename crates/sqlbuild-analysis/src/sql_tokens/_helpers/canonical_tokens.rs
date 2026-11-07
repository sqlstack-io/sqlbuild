//! The canonical token stream of SQL under SQLQuack's lossless tokenizer.

use sqlclean::sql_tokens::main::is_unquoted_word::is_unquoted_word;
use sqlclean::sql_tokens::main::name_positions::name_positions;
use sqlquack_syntax::lossless::models::TokenKind;

use crate::sql_tokens::_helpers::reserved_keywords::is_reserved_keyword;
use crate::sql_tokens::constants::{
    CALL_CLOSER, CALL_OPENER, COLUMN_LIST_RELATION_KEYWORDS, CTE_BODY_OPENERS, CTE_BODY_PREFIX,
    PLACEHOLDER_PREFIX,
};
use crate::sql_tokens::models::SqlquackFingerprintDialect;

/// Significant token texts with reserved keywords and called names, except `__` placeholders, upper-cased.
pub(crate) fn canonical_tokens(
    sql: &str,
    dialect: SqlquackFingerprintDialect,
) -> Result<Vec<String>, String> {
    let tokens = sqlquack_syntax::tokenize(sql, dialect.tokenizer)
        .map_err(|error| format!("SQL tokenization failed: {error}"))?;
    let raws: Vec<&str> = tokens
        .iter()
        .filter(|token| matches!(token.kind, TokenKind::Significant(_)))
        .map(|token| token.text(sql))
        .collect();
    let uppers: Vec<String> = raws.iter().map(|raw| raw.to_ascii_uppercase()).collect();
    let names: Vec<bool> = name_positions(&uppers);
    let mut texts: Vec<String> = Vec::with_capacity(raws.len());
    for (index, raw) in raws.iter().enumerate() {
        let calls = uppers
            .get(index + 1)
            .is_some_and(|next| next == CALL_OPENER)
            && !raw.starts_with(PLACEHOLDER_PREFIX)
            && !names_column_list(&uppers, index, dialect);
        let folds = !names[index]
            && is_unquoted_word(raw)
            && (calls || is_reserved_keyword(dialect.keywords, &uppers[index]));
        texts.push(if folds {
            uppers[index].clone()
        } else {
            (*raw).to_owned()
        });
    }
    Ok(texts)
}

/// Whether the word at `index` names the relation a column list follows, as in `cte(a, b) AS (`.
fn names_column_list(uppers: &[String], index: usize, dialect: SqlquackFingerprintDialect) -> bool {
    let previous: Option<&str> = index.checked_sub(1).map(|before| uppers[before].as_str());
    if previous.is_some_and(|word| {
        COLUMN_LIST_RELATION_KEYWORDS.contains(&word)
            || (is_unquoted_word(word) && !is_reserved_keyword(dialect.keywords, word))
    }) {
        return true;
    }
    let mut depth: usize = 0;
    for (offset, token) in uppers.iter().enumerate().skip(index + 1) {
        if token == CALL_OPENER {
            depth += 1;
        } else if token == CALL_CLOSER {
            depth -= 1;
            if depth == 0 {
                return uppers
                    .get(offset + 1)
                    .is_some_and(|next| next == CTE_BODY_PREFIX)
                    && uppers
                        .get(offset + 2)
                        .is_some_and(|next| CTE_BODY_OPENERS.contains(&next.as_str()));
            }
        }
    }
    false
}
