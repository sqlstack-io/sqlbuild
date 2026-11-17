use sqlbuild_sqltext::sql_scan::main::comment_end::comment_end;
use sqlbuild_sqltext::sql_scan::main::quote_end::quote_end;
use sqlbuild_sqltext::sql_scan::models::QuotePolicy;

const CANONICAL_NUMERIC_DECISIONS: [&str; 3] = ["-1", "0", "1"];

pub(crate) struct AuthoredDecision<'a> {
    pub(crate) authored_compact_sql: &'a str,
    pub(crate) comparison_sql: &'a str,
    pub(crate) equality: bool,
    pub(crate) output_name: Option<&'a str>,
    pub(crate) literal: &'a str,
    pub(crate) quote_policy: QuotePolicy,
}

pub(crate) fn compact_sql(sql: &str, policy: QuotePolicy) -> String {
    let bytes = sql.as_bytes();
    let mut compact = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        let byte = bytes[index];
        let unclosed_end = match comment_end(bytes, index) {
            Ok(Some(end)) => {
                index = end;
                continue;
            }
            Ok(None) if policy.is_quote(byte) => {
                quote_end(bytes, index, policy).unwrap_or(bytes.len())
            }
            Ok(None) => {
                if !byte.is_ascii_whitespace() {
                    compact.push(byte.to_ascii_lowercase());
                }
                index += 1;
                continue;
            }
            Err(_) => break,
        };
        compact.extend(
            bytes[index..unclosed_end]
                .iter()
                .map(u8::to_ascii_lowercase),
        );
        index = unclosed_end;
    }
    String::from_utf8(compact).unwrap_or_default()
}

/// `sql` with each whitespace run outside quoted text collapsed to one space.
pub(crate) fn single_spaced(sql: &str, policy: QuotePolicy) -> String {
    let bytes = sql.as_bytes();
    let mut spaced = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        let byte = bytes[index];
        if policy.is_quote(byte) {
            let end = quote_end(bytes, index, policy).unwrap_or(bytes.len());
            spaced.extend_from_slice(&bytes[index..end]);
            index = end;
        } else if byte.is_ascii_whitespace() {
            while bytes.get(index).is_some_and(u8::is_ascii_whitespace) {
                index += 1;
            }
            spaced.push(b' ');
        } else {
            spaced.push(byte);
            index += 1;
        }
    }
    String::from_utf8(spaced)
        .map(|value| value.trim().to_owned())
        .unwrap_or_else(|_| sql.to_owned())
}

pub(crate) fn is_authored_decision(decision: AuthoredDecision<'_>) -> bool {
    !CANONICAL_NUMERIC_DECISIONS.contains(&decision.literal)
        && decision
            .authored_compact_sql
            .contains(&compact_sql(decision.comparison_sql, decision.quote_policy))
        && !decision.output_name.is_some_and(|name| {
            decision.equality && name.ends_with(&format!("_{}", decision.literal))
        })
}
