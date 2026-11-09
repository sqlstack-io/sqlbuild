//! Closed token productions for test-planning facts; unrecognized input uses the full parser.

use std::collections::HashSet;

use sqlquack_parse::dialect_tokens;
use sqlquack_syntax::Dialect;
use sqlquack_syntax::tokens::models::{Token, TokenType};

use crate::compiler::_helpers::sql_tests::planning::{AnalysisTemplate, SqlTestPatterns};
use crate::relation_markers::constants::MARKER_NAME_PREFIX;
use crate::relation_markers::main::markers_as_names::markers_as_names;
use crate::relation_markers::types::MarkerCall;

/// Deepest query or scalar nesting the recognizer reads before it leaves the template to the parser.
const MAX_TEMPLATE_DEPTH: usize = 32;

/// Recognize bounded DuckDB/Snowflake productions; other syntax retains the full parser.
pub(crate) fn simple_analysis_template(
    query_sql: &str,
    dialect_name: &str,
    patterns: &SqlTestPatterns,
) -> Option<AnalysisTemplate> {
    let dialect: Dialect = match dialect_name {
        "duckdb" => Dialect::DuckDB,
        "snowflake" => Dialect::Snowflake,
        _ => return None,
    };
    if !query_sql.is_ascii() {
        return None;
    }
    let (sql, named) = match markers_as_names(query_sql, &patterns.lexical) {
        Ok(renamed) => renamed,
        Err(_unscannable) => return None,
    };
    let tokens: Vec<Token> = match dialect_tokens(&sql, dialect) {
        Ok(tokens) => tokens,
        Err(_untokenizable) => return None,
    };
    if !covers_all_but_whitespace(&sql, &tokens) {
        return None;
    }
    let mut reader = TemplateReader {
        tokens: &tokens,
        named: &named,
        position: 0,
    };
    let facts = reader.query(0)?;
    reader.take(TokenType::Semicolon);
    if reader.position != tokens.len() {
        return None;
    }
    Some(AnalysisTemplate {
        existing_cte_names: facts.names,
        marker_calls: facts.calls.into(),
    })
}

/// Whether only whitespace lies outside the tokens, so no comment or skipped text needs the parser.
fn covers_all_but_whitespace(sql: &str, tokens: &[Token]) -> bool {
    let bytes: &[u8] = sql.as_bytes();
    let mut cursor: usize = 0;
    for token in tokens {
        if !token.comments.is_empty()
            || !token.trailing_comments.is_empty()
            || token.span.start < cursor
            || token.span.end > bytes.len()
            || !bytes[cursor..token.span.start]
                .iter()
                .all(u8::is_ascii_whitespace)
        {
            return false;
        }
        cursor = token.span.end;
    }
    bytes[cursor..].iter().all(u8::is_ascii_whitespace)
}

struct QueryFacts {
    names: HashSet<String>,
    calls: Vec<MarkerCall>,
}

struct TemplateReader<'a> {
    tokens: &'a [Token],
    named: &'a [MarkerCall],
    position: usize,
}

impl<'a> TemplateReader<'a> {
    fn take(&mut self, kind: TokenType) -> bool {
        if self
            .tokens
            .get(self.position)
            .is_some_and(|token| token.token_type == kind)
        {
            self.position += 1;
            true
        } else {
            false
        }
    }

    fn require(&mut self, kind: TokenType) -> Option<()> {
        self.take(kind).then_some(())
    }

    fn identifier(&mut self) -> Option<&'a str> {
        let token = self.tokens.get(self.position)?;
        if !matches!(
            token.token_type,
            TokenType::Var | TokenType::Identifier | TokenType::QuotedIdentifier | TokenType::Final
        ) {
            return None;
        }
        self.position += 1;
        Some(&token.text)
    }

    fn query(&mut self, depth: usize) -> Option<QueryFacts> {
        if depth > MAX_TEMPLATE_DEPTH {
            return None;
        }
        let mut names: HashSet<String> = HashSet::new();
        let mut cte_calls: Vec<MarkerCall> = Vec::new();
        let has_with = self.take(TokenType::With);
        if has_with {
            loop {
                names.insert(self.identifier()?.to_owned());
                self.require(TokenType::As)?;
                self.require(TokenType::LParen)?;
                cte_calls.extend(self.query(depth + 1)?.calls);
                self.require(TokenType::RParen)?;
                if !self.take(TokenType::Comma) {
                    break;
                }
            }
        }
        let mut calls = self.select(depth)?;
        while self.take(TokenType::Union) {
            if has_with {
                return None;
            }
            self.require(TokenType::All)?;
            if self.take(TokenType::By) {
                let name = self.tokens.get(self.position)?;
                if name.token_type != TokenType::Var || !name.text.eq_ignore_ascii_case("name") {
                    return None;
                }
                self.position += 1;
            }
            calls.extend(self.select(depth)?);
        }
        calls.extend(cte_calls);
        Some(QueryFacts { names, calls })
    }

    fn select(&mut self, depth: usize) -> Option<Vec<MarkerCall>> {
        self.require(TokenType::Select)?;
        loop {
            if !self.take(TokenType::Star) {
                self.scalar(depth + 1)?;
                if self.take(TokenType::As) {
                    self.identifier()?;
                }
            }
            if !self.take(TokenType::Comma) {
                break;
            }
        }
        self.require(TokenType::From)?;
        let mut calls: Vec<MarkerCall> = self.relation()?.into_iter().collect();
        loop {
            let left = self.take(TokenType::Left);
            if !self.take(TokenType::Join) {
                if left {
                    return None;
                }
                break;
            }
            calls.extend(self.relation()?);
            self.require(TokenType::On)?;
            self.condition(depth + 1)?;
        }
        if self.take(TokenType::Where) {
            self.condition(depth + 1)?;
        }
        Some(calls)
    }

    /// A relation name and optional alias; a name `markers_as_names` gave a marker call reads back as it.
    fn relation(&mut self) -> Option<Option<MarkerCall>> {
        let name = self.identifier()?;
        let call =
            name.strip_prefix(MARKER_NAME_PREFIX)
                .and_then(|index| match index.parse::<usize>() {
                    Ok(index) => self.named.get(index).cloned(),
                    Err(_not_a_marker) => None,
                });
        if self.take(TokenType::As) {
            self.identifier()?;
        }
        Some(call)
    }

    fn condition(&mut self, depth: usize) -> Option<()> {
        self.scalar(depth)?;
        self.require(TokenType::Eq)?;
        self.scalar(depth)
    }

    fn scalar(&mut self, depth: usize) -> Option<()> {
        self.atom(depth)?;
        while self.take(TokenType::Plus)
            || self.take(TokenType::Dash)
            || self.take(TokenType::Star)
            || self.take(TokenType::Slash)
        {
            self.atom(depth)?;
        }
        Some(())
    }

    fn atom(&mut self, depth: usize) -> Option<()> {
        if depth > MAX_TEMPLATE_DEPTH {
            return None;
        }
        if self.take(TokenType::LParen) {
            self.scalar(depth + 1)?;
            return self.require(TokenType::RParen);
        }
        if self.take(TokenType::Cast) {
            self.require(TokenType::LParen)?;
            self.scalar(depth + 1)?;
            self.require(TokenType::As)?;
            let data_type = self.tokens.get(self.position)?;
            if !matches!(
                data_type.token_type,
                TokenType::Var | TokenType::Int | TokenType::Double
            ) || !["int", "integer", "double"]
                .iter()
                .any(|name| data_type.text.eq_ignore_ascii_case(name))
            {
                return None;
            }
            self.position += 1;
            return self.require(TokenType::RParen);
        }
        if let Some(token) = self.tokens.get(self.position)
            && token.token_type == TokenType::Number
        {
            if !token.text.bytes().any(|byte| byte.is_ascii_digit())
                || !token
                    .text
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || byte == b'.')
                || token.text.bytes().filter(|byte| *byte == b'.').count() > 1
            {
                return None;
            }
            self.position += 1;
            return Some(());
        }
        let name = self.identifier()?;
        if self.take(TokenType::LParen) {
            if !name.eq_ignore_ascii_case("coalesce") {
                return None;
            }
            self.scalar(depth + 1)?;
            while self.take(TokenType::Comma) {
                self.scalar(depth + 1)?;
            }
            self.require(TokenType::RParen)
        } else {
            if self.take(TokenType::Dot) {
                self.identifier()?;
            }
            Some(())
        }
    }
}
