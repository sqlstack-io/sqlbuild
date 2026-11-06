//! A comment- and quote-aware SQL token scan, and the `@...` constructs it finds.

use sqlbuild_render::macro_calls::main::opens_bare_call::opens_bare_call;
use sqlbuild_sqltext::header_strings::errors::{ProseError, TripleQuoteError};
use sqlbuild_sqltext::header_strings::main::closes_inside_text::closes_inside_text;
use sqlbuild_sqltext::header_strings::main::follows_description_key::follows_description_key;
use sqlbuild_sqltext::header_strings::main::opens_triple_quote::opens_triple_quote;
use sqlbuild_sqltext::header_strings::main::prose_end::prose_end;
use sqlbuild_sqltext::header_strings::main::triple_quoted_end::triple_quoted_end;
use sqlbuild_sqltext::sql_scan::main::dialect_non_code_end::dialect_non_code_end;
use sqlbuild_sqltext::sql_scan::models::LexicalSyntax;

use crate::sql_macros::constants::{HEADER_KEYWORD, SQL_KIND_WORD};
use crate::sql_macros::errors::LexError;

/// What one token is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TokenKind {
    /// An unquoted identifier or keyword.
    Word,
    /// A `"double"` or `` `backtick` `` quoted identifier, or a double-quoted header string.
    Quoted,
    /// A `'single'` or `$tag$dollar$tag$` quoted string.
    Text,
    Number,
    /// One of `( ) [ ] { } , . ;`.
    Punct,
    /// Any other operator character run, such as `:=`, `<>` or `@`.
    Operator,
}

/// One token and its byte span.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Token {
    pub(crate) kind: TokenKind,
    pub(crate) start: usize,
    pub(crate) end: usize,
}

impl Token {
    pub(crate) fn text<'a>(&self, sql: &'a str) -> &'a str {
        &sql[self.start..self.end]
    }

    /// Whether the token is the word `keyword`, ignoring case.
    pub(crate) fn is_word(&self, sql: &str, keyword: &str) -> bool {
        self.kind == TokenKind::Word && self.text(sql).eq_ignore_ascii_case(keyword)
    }

    /// Whether the token is the punctuation or operator `symbol`.
    pub(crate) fn is(&self, sql: &str, symbol: &str) -> bool {
        matches!(self.kind, TokenKind::Punct | TokenKind::Operator) && self.text(sql) == symbol
    }
}

const PUNCTUATION: &[u8] = b"()[]{},.;";

/// Where the quoted text opened at an offset ends.
type TextEnd<'a> = dyn Fn(&[u8], usize) -> Result<usize, LexError> + 'a;
/// Where a comment opened at an offset ends, `None` when none opens there.
type CommentEnd<'a> = dyn Fn(&[u8], usize) -> Result<Option<usize>, LexError> + 'a;
/// The `@word(...)` declaration forms that are not macro calls.
pub(crate) const DECLARATION_WORDS: [&str; 3] = ["const", "enum", "param"];

/// Every token of `sql`; text unclosed under doubled-quote escapes is reread with backslash ones.
pub(crate) fn tokens(sql: &str) -> Result<Vec<Token>, LexError> {
    tokens_with(sql, None)
}

/// Every token of `sql`, with quoted text ending where `syntax` ends it, or as [`tokens`] reads it.
pub(crate) fn tokens_with(
    sql: &str,
    syntax: Option<&LexicalSyntax>,
) -> Result<Vec<Token>, LexError> {
    if let Some(syntax) = syntax {
        let text_end = |bytes: &[u8], start: usize| match dialect_non_code_end(bytes, start, syntax)
        {
            Ok(Some(end)) => Ok(end),
            _ => Err(LexError::UnclosedQuote(start)),
        };
        let comment_end = |bytes: &[u8], start: usize| dialect_comment_end(bytes, start, syntax);
        return scanned_tokens(sql, &text_end, &comment_end);
    }
    match scanned_tokens(
        sql,
        &|bytes, start| quote_end(bytes, start, false).ok_or(LexError::UnclosedQuote(start)),
        &generic_comment_end,
    ) {
        Err(LexError::UnclosedQuote(start)) => scanned_tokens(
            sql,
            &|bytes, start| quote_end(bytes, start, true).ok_or(LexError::UnclosedQuote(start)),
            &generic_comment_end,
        )
        .map_err(|_| LexError::UnclosedQuote(start)),
        scanned => scanned,
    }
}

/// The end of a `--` or `/* */` comment opening at `start`, if one opens there.
fn generic_comment_end(bytes: &[u8], start: usize) -> Result<Option<usize>, LexError> {
    if bytes[start..].starts_with(b"--") {
        return Ok(Some(
            bytes[start..]
                .iter()
                .position(|byte| *byte == b'\n')
                .map_or(bytes.len(), |offset| start + offset + 1),
        ));
    }
    if bytes[start..].starts_with(b"/*") {
        return bytes[start + 2..]
            .windows(2)
            .position(|pair| pair == b"*/")
            .map(|offset| Some(start + 2 + offset + 2))
            .ok_or(LexError::UnclosedComment(start));
    }
    Ok(None)
}

/// The end of a comment opening at `start` under `syntax`'s comment rules, if one opens there.
fn dialect_comment_end(
    bytes: &[u8],
    start: usize,
    syntax: &LexicalSyntax,
) -> Result<Option<usize>, LexError> {
    if matches!(bytes[start], b'\'' | b'"' | b'`' | b'$') {
        return Ok(None);
    }
    dialect_non_code_end(bytes, start, syntax).map_err(|_| LexError::UnclosedComment(start))
}

/// Every token of `sql`; `text_end` and `comment_end` find where quoted text and comments end.
fn scanned_tokens(
    sql: &str,
    text_end: &TextEnd<'_>,
    comment_end: &CommentEnd<'_>,
) -> Result<Vec<Token>, LexError> {
    let bytes: &[u8] = sql.as_bytes();
    let mut found: Vec<Token> = Vec::new();
    let mut index: usize = 0;
    let mut header: HeaderScan = HeaderScan::Before;
    while index < bytes.len() {
        let byte: u8 = bytes[index];
        if byte.is_ascii_whitespace() {
            index += 1;
            continue;
        }
        if let Some(end) = comment_end(bytes, index)? {
            index = end;
            continue;
        }
        let start: usize = index;
        let in_header: bool = matches!(header, HeaderScan::Inside(_));
        let kind: TokenKind = match byte {
            b'\'' if in_header => {
                index = header_string_end(bytes, index)?;
                TokenKind::Text
            }
            b'"' if in_header => {
                index = header_string_end(bytes, index)?;
                TokenKind::Quoted
            }
            b'\'' => {
                index = text_end(bytes, index)?;
                TokenKind::Text
            }
            b'"' | b'`' => {
                index = text_end(bytes, index)?;
                TokenKind::Quoted
            }
            b'$' if dollar_end(bytes, index).is_some() => {
                index = dollar_end(bytes, index).ok_or(LexError::UnclosedQuote(start))?;
                TokenKind::Text
            }
            _ if byte.is_ascii_digit() => {
                index += 1;
                while index < bytes.len()
                    && (bytes[index].is_ascii_alphanumeric() || bytes[index] == b'.')
                {
                    index += 1;
                }
                TokenKind::Number
            }
            _ if is_word_start(sql, index) => {
                index = word_end(sql, index);
                TokenKind::Word
            }
            _ if PUNCTUATION.contains(&byte) => {
                index += 1;
                TokenKind::Punct
            }
            _ => {
                index += sql[index..].chars().next().map_or(1, char::len_utf8);
                while index < bytes.len()
                    && is_operator_byte(bytes[index])
                    && !(bytes[index] == b'-' && bytes.get(index + 1) == Some(&b'-'))
                    && !(bytes[index] == b'/' && bytes.get(index + 1) == Some(&b'*'))
                    && bytes[index - 1] != b'@'
                {
                    index += 1;
                }
                TokenKind::Operator
            }
        };
        let token: Token = Token {
            kind,
            start,
            end: index,
        };
        header = header.after(sql, &token, found.is_empty());
        found.push(token);
    }
    Ok(found)
}

/// Where a scan is relative to the `MACRO (...)` header that may start the text.
#[derive(Clone, Copy)]
enum HeaderScan {
    Before,
    /// The first token is `MACRO`.
    Keyword,
    /// Inside the header, at this bracket depth.
    Inside(usize),
    Outside,
}

impl HeaderScan {
    fn after(self, sql: &str, token: &Token, first: bool) -> Self {
        let opens: bool =
            token.kind == TokenKind::Punct && matches!(token.text(sql), "(" | "[" | "{");
        let closes: bool =
            token.kind == TokenKind::Punct && matches!(token.text(sql), ")" | "]" | "}");
        match self {
            Self::Before if first && token.is_word(sql, HEADER_KEYWORD) => Self::Keyword,
            Self::Keyword if token.is(sql, "(") => Self::Inside(1),
            Self::Inside(depth) if opens => Self::Inside(depth + 1),
            Self::Inside(1) if closes => Self::Outside,
            Self::Inside(depth) if closes => Self::Inside(depth - 1),
            Self::Inside(depth) => Self::Inside(depth),
            _ => Self::Outside,
        }
    }
}

/// The lexing error of a prose string that does not end where it should.
fn prose_lex_error(error: ProseError) -> LexError {
    match error {
        ProseError::Unterminated(start) => LexError::UnclosedQuote(start),
        ProseError::DoubleQuoteInside(position) => LexError::QuoteInside(position),
        ProseError::SingleQuoteInside(position) => LexError::SingleQuoteInside(position),
        ProseError::TripleQuoteInside(position) => LexError::TripleQuoteInside(position),
        ProseError::UnclosedTripleQuote(start) => LexError::UnclosedTripleQuote(start),
    }
}

/// The end of the header string at `start`: a description by prose rules, else as a SQL string.
fn header_string_end(bytes: &[u8], start: usize) -> Result<usize, LexError> {
    if follows_description_key(bytes, start) {
        return prose_end(bytes, start).map_err(prose_lex_error);
    }
    if opens_triple_quote(bytes, start) {
        let (content_end, end) = triple_quoted_end(bytes, start).map_err(|error| match error {
            TripleQuoteError::Unclosed => LexError::UnclosedTripleQuote(start),
            TripleQuoteError::Inside(position) => LexError::TripleQuoteInside(position),
        })?;
        if closes_inside_text(bytes, end) {
            return Err(LexError::TripleQuoteInside(content_end));
        }
        return Ok(end);
    }
    quoted_end(bytes, start)
        .or_else(|| quote_end(bytes, start, true))
        .ok_or(LexError::UnclosedQuote(start))
}

fn is_operator_byte(byte: u8) -> bool {
    matches!(
        byte,
        b'<' | b'>'
            | b'='
            | b'!'
            | b':'
            | b'|'
            | b'&'
            | b'+'
            | b'-'
            | b'*'
            | b'/'
            | b'%'
            | b'^'
            | b'~'
    )
}

fn is_word_start(sql: &str, index: usize) -> bool {
    sql[index..]
        .chars()
        .next()
        .is_some_and(|character| character == '_' || character.is_alphabetic())
}

/// The byte offset after the identifier starting at `index`.
pub(crate) fn word_end(sql: &str, index: usize) -> usize {
    let mut end: usize = index;
    for character in sql[index..].chars() {
        if character == '_' || character == '$' || character.is_alphanumeric() {
            end += character.len_utf8();
        } else {
            break;
        }
    }
    end
}

/// The byte offset after the quoted text starting at `start`; doubled quotes escape.
pub(crate) fn quoted_end(bytes: &[u8], start: usize) -> Option<usize> {
    let quote: u8 = bytes[start];
    let mut index: usize = start + 1;
    loop {
        index += bytes.get(index..)?.iter().position(|byte| *byte == quote)?;
        if bytes.get(index + 1) == Some(&quote) {
            index += 2;
            continue;
        }
        return Some(index + 1);
    }
}

/// The byte offset after the quoted text at `start`, where a backslash may escape the next byte.
fn quote_end(bytes: &[u8], start: usize, backslash_escapes: bool) -> Option<usize> {
    if !backslash_escapes {
        return quoted_end(bytes, start);
    }
    let quote: u8 = bytes[start];
    let mut index: usize = start + 1;
    while let Some(byte) = bytes.get(index) {
        if *byte == b'\\' {
            index += 2;
            continue;
        }
        if *byte == quote {
            if bytes.get(index + 1) == Some(&quote) {
                index += 2;
                continue;
            }
            return Some(index + 1);
        }
        index += 1;
    }
    None
}

/// The byte offset after a `$tag$...$tag$` string starting at `start`, if one starts there.
fn dollar_end(bytes: &[u8], start: usize) -> Option<usize> {
    if start
        .checked_sub(1)
        .is_some_and(|previous| bytes[previous].is_ascii_alphanumeric() || bytes[previous] == b'_')
    {
        return None;
    }
    let tag_end: usize = bytes[start + 1..]
        .iter()
        .position(|byte| !(byte.is_ascii_alphanumeric() || *byte == b'_'))
        .map_or(bytes.len(), |offset| start + 1 + offset);
    if bytes.get(tag_end) != Some(&b'$') || bytes.get(start + 1).is_some_and(u8::is_ascii_digit) {
        return None;
    }
    let delimiter: &[u8] = &bytes[start..=tag_end];
    bytes[tag_end + 1..]
        .windows(delimiter.len())
        .position(|window| window == delimiter)
        .map(|offset| tag_end + 1 + offset + delimiter.len())
}

/// The index of the token closing the bracket token at `open`, if it closes.
pub(crate) fn closing_token(sql: &str, tokens: &[Token], open: usize) -> Option<usize> {
    let mut depth: usize = 0;
    for (index, token) in tokens.iter().enumerate().skip(open) {
        if token.kind != TokenKind::Punct {
            continue;
        }
        match token.text(sql) {
            "(" | "[" | "{" => depth += 1,
            ")" | "]" | "}" => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(index);
                }
            }
            _ => {}
        }
    }
    None
}

/// The one-based line and column of byte `offset` in `text`, columns counting characters.
pub(crate) fn line_column(text: &str, offset: usize) -> (usize, usize) {
    let before: &str = &text[..offset.min(text.len())];
    let line: usize = before.matches('\n').count() + 1;
    let line_start: usize = before.rfind('\n').map_or(0, |newline| newline + 1);
    (line, before[line_start..].chars().count() + 1)
}

/// One `@` construct and its byte span.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum AtConstruct {
    /// `@sql.name(...)`: the span of the whole call and of its arguments.
    Sql {
        name: String,
        start: usize,
        end: usize,
        arguments: (usize, usize),
    },
    /// `@py.name(...)`.
    Python {
        name: String,
        start: usize,
        end: usize,
    },
    /// `@name(...)` without a kind.
    Bare { name: String, start: usize },
}

/// Every SQL macro call, Python macro call and bare call in `sql`, top-level and nested.
pub(crate) fn at_constructs(
    sql: &str,
    syntax: Option<&LexicalSyntax>,
) -> Result<Vec<AtConstruct>, LexError> {
    if !sql.contains('@') {
        return Ok(Vec::new());
    }
    let scanned: Vec<Token> = tokens_with(sql, syntax)?;
    let mut found: Vec<AtConstruct> = Vec::new();
    for (index, token) in scanned.iter().enumerate() {
        if !token.is(sql, "@") {
            continue;
        }
        let Some(word) = scanned
            .get(index + 1)
            .filter(|word| word.kind == TokenKind::Word && word.start == token.end)
        else {
            continue;
        };
        let word_text: &str = word.text(sql);
        if DECLARATION_WORDS.contains(&word_text) {
            continue;
        }
        let kinded: bool = matches!(word_text, "sql" | "py")
            && scanned
                .get(index + 2)
                .is_some_and(|dot| dot.is(sql, ".") && dot.start == word.end);
        if kinded {
            let Some(name) = scanned
                .get(index + 3)
                .filter(|name| name.kind == TokenKind::Word)
            else {
                continue;
            };
            let Some(open) = scanned.get(index + 4).filter(|open| open.is(sql, "(")) else {
                continue;
            };
            let Some(close) = closing_token(sql, &scanned, index + 4) else {
                continue;
            };
            let end: usize = scanned[close].end;
            let name_text: String = name.text(sql).to_owned();
            found.push(if word_text == SQL_KIND_WORD {
                AtConstruct::Sql {
                    name: name_text,
                    start: token.start,
                    end,
                    arguments: (open.end, scanned[close].start),
                }
            } else {
                AtConstruct::Python {
                    name: name_text,
                    start: token.start,
                    end,
                }
            });
            continue;
        }
        if opens_bare_call(sql, word.end) {
            found.push(AtConstruct::Bare {
                name: word_text.to_owned(),
                start: token.start,
            });
        }
    }
    Ok(found)
}

/// Each outer SQL macro call of `sql` and whether it stands in a Python macro call's arguments.
pub(crate) fn outer_sql_calls(constructs: &[AtConstruct]) -> Vec<(&AtConstruct, bool)> {
    let mut found: Vec<(&AtConstruct, bool)> = Vec::new();
    let mut sql_until: usize = 0;
    let mut python_until: usize = 0;
    for construct in constructs {
        match construct {
            AtConstruct::Sql { start, end, .. } => {
                if *start < sql_until {
                    continue;
                }
                sql_until = *end;
                found.push((construct, *start < python_until));
            }
            AtConstruct::Python { start, end, .. } => {
                if *start >= python_until && *start >= sql_until {
                    python_until = *end;
                }
            }
            AtConstruct::Bare { .. } => {}
        }
    }
    found
}
