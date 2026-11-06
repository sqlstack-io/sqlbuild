//! Batched tokenization for authored SQLBuild MODEL headers.

use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use rayon::{ThreadPool, ThreadPoolBuilder};

use crate::compiler::models::{AuthoredValue, NestingFailure};
use crate::constants::{
    COLUMNS_KEY, CONSTANT_CALL, INLINE_SQL_HOOK, MAX_HEADER_NESTING_DEPTH, OUTSIDE_KEY, SQL_HOOK,
    THRESHOLDS_KEY, TYPE_KEY,
};
use crate::header_strings::constants::{
    AUDITS_KEY, DESCRIPTION_KEY, POSITION_SUFFIX, TRIPLE_QUOTE_INSIDE, TRIPLE_QUOTE_LENGTH,
    UNCLOSED_TRIPLE_QUOTE,
};
use crate::header_strings::errors::TripleQuoteError;
use crate::header_strings::main::closes_inside_text::closes_inside_text;
use crate::header_strings::main::follows_description_key::follows_description_key;
use crate::header_strings::main::opens_triple_quote::opens_triple_quote;
use crate::header_strings::main::prose_end::prose_end;
use crate::header_strings::main::prose_error_message::prose_error_message;
use crate::header_strings::main::prose_string::prose_string;
use crate::header_strings::main::triple_quoted_end::triple_quoted_end;
use sqlbuild_core::constants::{CLOSE_PAREN, OPEN_PAREN};

pub(crate) const MAX_TOKENIZER_WORKERS: usize = 4;
pub(crate) const TOKENIZER_WORKER_STACK_BYTES: usize = 16 * 1024 * 1024;

pub(crate) const END_TOKEN: u8 = 0;
pub(crate) const WORD_TOKEN: u8 = 1;
pub(crate) const STRING_TOKEN: u8 = 2;
pub(crate) const SYMBOL_TOKEN: u8 = 3;

pub(crate) type HeaderToken = (u8, String, usize);
pub(crate) type HeaderColumnOffset = (String, usize, usize);
pub(crate) type HeaderParseResult = (
    Option<AuthoredValue>,
    Option<Vec<HeaderColumnOffset>>,
    Option<String>,
);

pub(crate) fn parse_batch(headers: &[String]) -> Result<Vec<HeaderParseResult>, String> {
    let pool = build_tokenizer_pool(headers.len())?;
    Ok(pool.install(|| headers.par_iter().map(|header| parse_one(header)).collect()))
}

pub(crate) fn parse_one(header: &str) -> HeaderParseResult {
    let characters: Vec<char> = header.chars().collect();
    match tokenize(&characters).and_then(|tokens| HeaderParser::parse(tokens, &characters)) {
        Ok((value, offsets)) => (Some(value), Some(offsets), None),
        Err(error) => (None, None, Some(error)),
    }
}

pub(crate) fn tokenize_one(header: &str) -> Result<Vec<HeaderToken>, String> {
    let characters: Vec<char> = header.chars().collect();
    tokenize(&characters)
}

pub(crate) fn build_tokenizer_pool(header_count: usize) -> Result<ThreadPool, String> {
    ThreadPoolBuilder::new()
        .num_threads(header_count.clamp(1, MAX_TOKENIZER_WORKERS))
        .stack_size(TOKENIZER_WORKER_STACK_BYTES)
        .build()
        .map_err(|error| error.to_string())
}

struct HeaderParser<'a> {
    tokens: Vec<HeaderToken>,
    /// The header text the token positions index, which prose values are read from.
    characters: &'a [char],
    index: usize,
    column_offsets: Vec<HeaderColumnOffset>,
    /// The open containers, at most `MAX_HEADER_NESTING_DEPTH`, which bounds the recursion.
    depth: usize,
}

#[derive(Clone, Copy, Default)]
struct ParseValueOptions {
    threshold_policy: bool,
    allow_outside: bool,
    nested_columns: bool,
    nested_metadata: bool,
    record_nested_columns: bool,
    /// The value of an `audits` key, whose audit calls have a prose `description` option.
    audit_list: bool,
    /// An item of an `audits` list.
    audit_call: bool,
}

#[derive(Clone, Copy, Default)]
struct ParseMapOptions<'a> {
    end: Option<&'a str>,
    threshold_policy: bool,
    allow_outside: bool,
    column_entries: bool,
    column_metadata: bool,
    record_column_entries: bool,
    /// Whether a `description` string here is prose, stored folded.
    prose_descriptions: bool,
}

impl<'a> HeaderParser<'a> {
    fn parse(
        tokens: Vec<HeaderToken>,
        characters: &'a [char],
    ) -> Result<(AuthoredValue, Vec<HeaderColumnOffset>), String> {
        let mut parser = Self {
            tokens,
            characters,
            index: 0,
            column_offsets: Vec::new(),
            depth: 0,
        };
        let value = if parser.peek().0 == END_TOKEN {
            AuthoredValue::Map(Vec::new())
        } else {
            AuthoredValue::Map(parser.parse_map(ParseMapOptions {
                prose_descriptions: true,
                ..ParseMapOptions::default()
            })?)
        };
        parser.expect_end()?;
        Ok((value, parser.column_offsets))
    }

    fn parse_map(
        &mut self,
        options: ParseMapOptions<'_>,
    ) -> Result<Vec<(String, AuthoredValue)>, String> {
        let mut values: Vec<(String, AuthoredValue)> = Vec::new();
        while !self.at_end_symbol(options.end) {
            if self.match_symbol(",") {
                continue;
            }
            let key_token = self.peek().clone();
            let key = if options.column_entries {
                self.consume_column_key()?
            } else {
                self.consume_key()?
            };
            if values.iter().any(|(existing, _)| existing == &key) {
                return Err(format!("duplicate key '{key}'"));
            }
            if self.match_symbol(":") {
                return Err(format!(
                    "unexpected ':' after key '{key}'; use SQLBuild syntax '{key} value'"
                ));
            }
            if self.at_end_symbol(options.end) || self.peek().0 == END_TOKEN {
                return Err(format!(
                    "unexpected token '{key}' without a value; quote values with spaces"
                ));
            }
            if options.column_entries && options.record_column_entries {
                self.column_offsets
                    .push((key.clone(), key_token.2, key.chars().count()));
            }
            let value = if options.allow_outside && key == OUTSIDE_KEY {
                self.parse_outside()?
            } else if matches!(key.as_str(), "pre_hooks" | "post_hooks") {
                self.parse_hook_field(&key)?
            } else if options.column_metadata && key == TYPE_KEY {
                self.parse_type()?
            } else if options.prose_descriptions
                && key == DESCRIPTION_KEY
                && self.peek().0 == STRING_TOKEN
            {
                let position: usize = self.advance().2;
                AuthoredValue::String(prose_string(self.characters, position)?.value)
            } else {
                self.parse_value(ParseValueOptions {
                    threshold_policy: key == THRESHOLDS_KEY,
                    allow_outside: options.threshold_policy
                        && matches!(key.as_str(), "warn" | "error"),
                    nested_columns: key == COLUMNS_KEY,
                    nested_metadata: options.column_entries,
                    record_nested_columns: options.end.is_none() && key == COLUMNS_KEY,
                    audit_list: key == AUDITS_KEY,
                    audit_call: false,
                })?
            };
            values.push((key, value));
            self.match_symbol(",");
        }
        if let Some(symbol) = options.end {
            self.consume_symbol(symbol)?;
        }
        Ok(values)
    }

    fn parse_outside(&mut self) -> Result<AuthoredValue, String> {
        let lower = self.parse_value(ParseValueOptions::default())?;
        if self.at_end_symbol(Some(")")) {
            return Err("outside threshold requires lower and upper values".to_owned());
        }
        let upper = self.parse_value(ParseValueOptions::default())?;
        Ok(AuthoredValue::Tuple(vec![lower, upper]))
    }

    fn parse_type(&mut self) -> Result<AuthoredValue, String> {
        let token = self.peek().clone();
        if token.0 != WORD_TOKEN
            || self
                .tokens
                .get(self.index + 1)
                .is_none_or(|next| next.0 != SYMBOL_TOKEN || next.1 != OPEN_PAREN)
        {
            return self.parse_value(ParseValueOptions::default());
        }
        let mut value = self.advance().1.clone();
        value.push_str(&self.advance().1);
        let mut depth = 1_usize;
        while depth > 0 {
            let parameter = self.peek().clone();
            if parameter.0 == END_TOKEN {
                return Err(format!("unterminated SQL type at position {}", token.2));
            }
            if parameter.0 == STRING_TOKEN {
                return Err(format!(
                    "quoted SQL type parameters require the whole type to be quoted at position {}",
                    parameter.2
                ));
            }
            self.advance();
            value.push_str(&parameter.1);
            if parameter.0 == SYMBOL_TOKEN && parameter.1 == OPEN_PAREN {
                depth += 1;
            } else if parameter.0 == SYMBOL_TOKEN && parameter.1 == CLOSE_PAREN {
                depth -= 1;
            }
        }
        Ok(AuthoredValue::String(value))
    }

    fn parse_value(&mut self, options: ParseValueOptions) -> Result<AuthoredValue, String> {
        let token = self.peek().clone();
        if token.0 == STRING_TOKEN {
            self.advance();
            return Ok(AuthoredValue::String(token.1));
        }
        if token.0 == WORD_TOKEN {
            self.advance();
            if self.peek().0 == SYMBOL_TOKEN && self.peek().1 == OPEN_PAREN {
                self.advance();
                return self.nested(token.2, |parser| parser.parse_call(token, options));
            }
            return Ok(parse_word(token.1));
        }
        let items: ParseValueOptions = ParseValueOptions {
            audit_call: options.audit_list,
            ..ParseValueOptions::default()
        };
        if self.match_symbol("[") {
            return self.nested(token.2, |parser| {
                Ok(AuthoredValue::List(parser.parse_sequence("]", items)?))
            });
        }
        if self.match_symbol("{") {
            return self.nested(token.2, |parser| {
                Ok(AuthoredValue::Set(parser.parse_sequence("}", items)?))
            });
        }
        if self.match_symbol("(") {
            return self.nested(token.2, |parser| {
                Ok(AuthoredValue::Map(parser.parse_map(ParseMapOptions {
                    end: Some(")"),
                    threshold_policy: options.threshold_policy,
                    allow_outside: options.allow_outside,
                    column_entries: options.nested_columns,
                    column_metadata: options.nested_metadata,
                    record_column_entries: options.record_nested_columns,
                    prose_descriptions: options.nested_metadata,
                })?))
            });
        }
        Err(format!("expected value at position {}", token.2))
    }

    /// The value after a consumed `name(`; a plain call nests a map in a map, two levels.
    fn parse_call(
        &mut self,
        (_, name, position): HeaderToken,
        options: ParseValueOptions,
    ) -> Result<AuthoredValue, String> {
        if matches!(name.as_str(), "__ref" | "__seed" | "__source") {
            return self.parse_relation(&name);
        }
        if matches!(name.as_str(), "inline_sql" | "sql" | "python") {
            return self.parse_hook(&name, position);
        }
        if name == CONSTANT_CALL {
            return Ok(AuthoredValue::TypedConstant(self.parse_map(
                ParseMapOptions {
                    end: Some(")"),
                    ..ParseMapOptions::default()
                },
            )?));
        }
        let arguments: Vec<(String, AuthoredValue)> = self.nested(position, |parser| {
            parser.parse_map(ParseMapOptions {
                end: Some(")"),
                threshold_policy: options.threshold_policy,
                allow_outside: options.allow_outside,
                prose_descriptions: options.audit_call,
                ..ParseMapOptions::default()
            })
        })?;
        Ok(AuthoredValue::Map(vec![(
            name,
            AuthoredValue::Map(arguments),
        )]))
    }

    /// Parse one container opened at `position`, refusing to nest deeper than the limit.
    fn nested<T>(
        &mut self,
        position: usize,
        parse: impl FnOnce(&mut Self) -> Result<T, String>,
    ) -> Result<T, String> {
        if self.depth >= MAX_HEADER_NESTING_DEPTH {
            return Err(nesting_error(position));
        }
        self.depth += 1;
        let value = parse(self);
        self.depth -= 1;
        value
    }

    fn parse_sequence(
        &mut self,
        end: &str,
        items: ParseValueOptions,
    ) -> Result<Vec<AuthoredValue>, String> {
        let mut values: Vec<AuthoredValue> = Vec::new();
        while !self.at_end_symbol(Some(end)) {
            if self.match_symbol(",") {
                continue;
            }
            values.push(self.parse_value(items)?);
            self.match_symbol(",");
        }
        self.consume_symbol(end)?;
        Ok(values)
    }

    fn parse_hook_field(&mut self, field: &str) -> Result<AuthoredValue, String> {
        let position: usize = self.peek().2;
        if !self.match_symbol("[") {
            return Err(format!(
                "{field} must be a list of typed inline_sql(...), sql(...), or python(...) hook entries at position {position}"
            ));
        }
        self.nested(position, |parser| parser.parse_hook_entries(field))
    }

    fn parse_hook_entries(&mut self, field: &str) -> Result<AuthoredValue, String> {
        let mut values: Vec<AuthoredValue> = Vec::new();
        while !self.at_end_symbol(Some("]")) {
            if self.match_symbol(",") {
                continue;
            }
            values.push(self.parse_hook_entry(field)?);
            self.match_symbol(",");
        }
        self.consume_symbol("]")?;
        Ok(AuthoredValue::List(values))
    }

    fn parse_hook_entry(&mut self, field: &str) -> Result<AuthoredValue, String> {
        let token = self.peek().clone();
        if token.0 == STRING_TOKEN {
            self.advance();
            return Ok(AuthoredValue::String(token.1));
        }
        let error = || {
            format!(
                "{field} entries must use typed inline_sql(...), sql(...), or python(...) hook syntax"
            )
        };
        if token.0 != WORD_TOKEN {
            return Err(error());
        }
        self.advance();
        if self.peek().0 != SYMBOL_TOKEN || self.peek().1 != OPEN_PAREN {
            return Err(error());
        }
        self.advance();
        if !matches!(token.1.as_str(), "inline_sql" | "sql" | "python") {
            return Err(error());
        }
        self.nested(token.2, |parser| parser.parse_hook(&token.1, token.2))
    }

    fn parse_relation(&mut self, name: &str) -> Result<AuthoredValue, String> {
        let token = self.peek().clone();
        if token.0 != STRING_TOKEN {
            return Err(format!(
                "{name}(...) requires a double-quoted relation name"
            ));
        }
        self.advance();
        self.consume_symbol(")")?;
        Ok(AuthoredValue::String(format!(
            "{name}(\"{}\")",
            token.1.replace('"', "\\\"")
        )))
    }

    /// The hook after `name(` at `position`; its keyword arguments are one more level.
    fn parse_hook(&mut self, name: &str, position: usize) -> Result<AuthoredValue, String> {
        if name == INLINE_SQL_HOOK {
            let token = self.peek().clone();
            if token.0 != STRING_TOKEN {
                return Err("inline_sql(...) requires a quoted SQL string".to_owned());
            }
            self.advance();
            if self.match_symbol(",") {
                return Err("inline_sql(...) does not accept additional arguments".to_owned());
            }
            self.consume_symbol(")")?;
            return Ok(AuthoredValue::InlineSqlHook(token.1));
        }
        let token = self.peek().clone();
        if token.0 != STRING_TOKEN {
            return Err(format!("{name}(...) requires a quoted hook name"));
        }
        self.advance();
        if token.1.chars().all(char::is_whitespace) {
            return Err(format!("{name}(...) requires a non-empty hook name"));
        }
        let kwargs = self.nested(position, Self::parse_hook_kwargs)?;
        if name == SQL_HOOK {
            Ok(AuthoredValue::NamedSqlHook(token.1, kwargs))
        } else {
            Ok(AuthoredValue::PythonHook(token.1, kwargs))
        }
    }

    fn parse_hook_kwargs(&mut self) -> Result<Vec<(String, AuthoredValue)>, String> {
        let mut values: Vec<(String, AuthoredValue)> = Vec::new();
        while self.match_symbol(",") {
            if self.at_end_symbol(Some(")")) {
                break;
            }
            let key = self.consume_key()?;
            if values.iter().any(|(existing, _)| existing == &key) {
                return Err(format!("duplicate hook argument '{key}'"));
            }
            self.consume_symbol(":")?;
            values.push((key, self.parse_value(ParseValueOptions::default())?));
        }
        self.consume_symbol(")")?;
        Ok(values)
    }

    fn consume_key(&mut self) -> Result<String, String> {
        let token = self.peek().clone();
        if token.0 != WORD_TOKEN {
            return Err(format!("expected key at position {}", token.2));
        }
        self.advance();
        Ok(token.1)
    }

    /// A column name: a bare word, or a double-quoted identifier kept with its quotes.
    fn consume_column_key(&mut self) -> Result<String, String> {
        let token = self.peek().clone();
        let quoted: bool = token.0 == STRING_TOKEN
            && self.characters.get(token.2) == Some(&'"')
            && !opens_triple_quote(self.characters, token.2);
        if !quoted {
            return self.consume_key();
        }
        self.advance();
        Ok(format!("\"{}\"", token.1))
    }

    fn at_end_symbol(&self, symbol: Option<&str>) -> bool {
        let token = self.peek();
        symbol.map_or(token.0 == END_TOKEN, |expected| {
            token.0 == SYMBOL_TOKEN && token.1 == expected
        })
    }

    fn match_symbol(&mut self, symbol: &str) -> bool {
        if self.peek().0 == SYMBOL_TOKEN && self.peek().1 == symbol {
            self.advance();
            true
        } else {
            false
        }
    }

    fn consume_symbol(&mut self, symbol: &str) -> Result<(), String> {
        if self.match_symbol(symbol) {
            Ok(())
        } else {
            Err(format!("expected '{symbol}' at position {}", self.peek().2))
        }
    }

    fn expect_end(&self) -> Result<(), String> {
        let token = self.peek();
        if token.0 == END_TOKEN {
            Ok(())
        } else {
            Err(format!(
                "unexpected token '{}' at position {}",
                token.1, token.2
            ))
        }
    }

    fn peek(&self) -> &HeaderToken {
        &self.tokens[self.index]
    }

    fn advance(&mut self) -> &HeaderToken {
        let token = &self.tokens[self.index];
        self.index += 1;
        token
    }
}

fn nesting_error(position: usize) -> String {
    format!("{}{position}", nesting_error_prefix())
}

fn nesting_error_prefix() -> String {
    format!("values nest deeper than {MAX_HEADER_NESTING_DEPTH} levels at position ")
}

/// The located nesting failure for `error` of `header` starting on `header_line`, if it is one.
pub(crate) fn nesting_failure(
    error: &str,
    header: &str,
    header_line: usize,
) -> Option<NestingFailure> {
    let digits: &str = error.strip_prefix(nesting_error_prefix().as_str())?;
    if digits.is_empty() {
        return None;
    }
    let position: usize = digits.bytes().try_fold(0_usize, |position, digit| {
        digit
            .is_ascii_digit()
            .then(|| {
                position
                    .checked_mul(10)?
                    .checked_add(usize::from(digit - b'0'))
            })
            .flatten()
    })?;
    Some(NestingFailure {
        line: header_line
            + header
                .chars()
                .take(position)
                .filter(|character| *character == '\n')
                .count(),
        message: format!("values nest deeper than {MAX_HEADER_NESTING_DEPTH} levels"),
        help: format!(
            "flatten the value so it nests at most {MAX_HEADER_NESTING_DEPTH} levels deep"
        ),
    })
}

fn parse_word(value: String) -> AuthoredValue {
    match value.as_str() {
        "true" => AuthoredValue::Boolean(true),
        "false" => AuthoredValue::Boolean(false),
        "null" => AuthoredValue::Null,
        _ => AuthoredValue::BareWord(value),
    }
}

/// The tokens of `characters`; when they fail after a misquoted description, that error.
fn tokenize(characters: &[char]) -> Result<Vec<HeaderToken>, String> {
    let mut scan: TokenScan<'_> = TokenScan {
        characters,
        prose_error: None,
    };
    scan.tokens()
        .map_err(|error| scan.prose_error.take().unwrap_or(error))
}

/// One tokenization of a header, remembering the first description that is misquoted as prose.
struct TokenScan<'a> {
    characters: &'a [char],
    prose_error: Option<String>,
}

impl TokenScan<'_> {
    fn tokens(&mut self) -> Result<Vec<HeaderToken>, String> {
        let characters: &[char] = self.characters;
        let mut tokens: Vec<HeaderToken> = Vec::new();
        let mut index = 0;
        while index < characters.len() {
            let character = characters[index];
            if char::is_whitespace(character) {
                index += 1;
                continue;
            }
            if starts_comment(characters, index) {
                index = comment_end(characters, index)?;
                continue;
            }
            if starts_template(characters, index) {
                let template_end = find_character(characters, '}', index + 2)
                    .ok_or_else(|| format!("unterminated template value at position {index}"))?;
                tokens.push((
                    WORD_TOKEN,
                    characters[index..=template_end].iter().collect(),
                    index,
                ));
                index = template_end + 1;
                continue;
            }
            if is_symbol(character) || character == ':' {
                tokens.push((SYMBOL_TOKEN, character.to_string(), index));
                index += 1;
                continue;
            }
            if opens_triple_quote(characters, index) {
                let (content_end, end) = triple_quoted_string(characters, index)?;
                tokens.push((
                    STRING_TOKEN,
                    characters[index + TRIPLE_QUOTE_LENGTH..content_end]
                        .iter()
                        .collect(),
                    index,
                ));
                index = end;
                continue;
            }
            if matches!(character, '\'' | '"') {
                let (value, next_index) = self.quoted_token(index)?;
                tokens.push((STRING_TOKEN, value, index));
                index = next_index;
                continue;
            }

            let start = index;
            while index < characters.len() {
                if starts_template(characters, index) {
                    let template_end =
                        find_character(characters, '}', index + 2).ok_or_else(|| {
                            format!("unterminated template value at position {index}")
                        })?;
                    index = template_end + 1;
                    continue;
                }
                let next = characters[index];
                if char::is_whitespace(next)
                    || is_symbol(next)
                    || next == ':'
                    || starts_comment(characters, index)
                {
                    break;
                }
                if matches!(next, '\'' | '"') {
                    let quote_name = if next == '\'' { "single" } else { "double" };
                    return Err(format!(
                        "unexpected {quote_name} quote inside bare value at position {index}; quote the whole value"
                    ));
                }
                index += 1;
            }
            if start == index {
                return Err(format!(
                    "unexpected character '{character}' at position {start}"
                ));
            }
            tokens.push((WORD_TOKEN, characters[start..index].iter().collect(), start));
        }
        tokens.push((END_TOKEN, String::new(), characters.len()));
        Ok(tokens)
    }

    /// The value and end of the quoted string at `start`; a description's ends where prose ends.
    fn quoted_token(&mut self, start: usize) -> Result<(String, usize), String> {
        let characters: &[char] = self.characters;
        let read: Result<(String, usize), String> = read_quoted_string(characters, start);
        if !follows_description_key(characters, start) {
            return read;
        }
        let end: usize = match prose_end(characters, start) {
            Ok(end) => end,
            Err(error) => {
                self.prose_error
                    .get_or_insert_with(|| prose_error_message(error, characters[start]));
                return read;
            }
        };
        match read {
            Ok((value, read_end)) if read_end == end => Ok((value, end)),
            _ => Ok((characters[start + 1..end - 1].iter().collect(), end)),
        }
    }
}

/// The text end and end of the `"""` string at `start`, checked not to close inside its text.
fn triple_quoted_string(characters: &[char], start: usize) -> Result<(usize, usize), String> {
    let (content_end, end) = triple_quoted_end(characters, start).map_err(|error| match error {
        TripleQuoteError::Unclosed => format!("{UNCLOSED_TRIPLE_QUOTE}{POSITION_SUFFIX}{start}"),
        TripleQuoteError::Inside(position) => {
            format!("{TRIPLE_QUOTE_INSIDE}{POSITION_SUFFIX}{position}")
        }
    })?;
    if closes_inside_text(characters, end) {
        return Err(format!(
            "{TRIPLE_QUOTE_INSIDE}{POSITION_SUFFIX}{content_end}"
        ));
    }
    Ok((content_end, end))
}

fn read_quoted_string(characters: &[char], start: usize) -> Result<(String, usize), String> {
    let quote = characters[start];
    let quote_name = if quote == '\'' { "single" } else { "double" };
    let mut value = String::new();
    let mut index = start + 1;
    while index < characters.len() {
        let character = characters[index];
        if character == '\\' {
            if index + 1 >= characters.len() {
                return Err(format!("unterminated escape at position {index}"));
            }
            value.push(characters[index + 1]);
            index += 2;
            continue;
        }
        if character == quote {
            return Ok((value, index + 1));
        }
        value.push(character);
        index += 1;
    }
    Err(format!(
        "unterminated {quote_name}-quoted string at position {start}"
    ))
}

fn starts_comment(characters: &[char], index: usize) -> bool {
    matches!(
        (characters.get(index), characters.get(index + 1)),
        (Some('-'), Some('-')) | (Some('/'), Some('*'))
    )
}

fn comment_end(characters: &[char], start: usize) -> Result<usize, String> {
    if characters[start] == '-' {
        return Ok(find_character(characters, '\n', start).map_or(characters.len(), |end| end + 1));
    }
    let mut index = start + 2;
    while index + 1 < characters.len() {
        if characters[index] == '*' && characters[index + 1] == '/' {
            return Ok(index + 2);
        }
        index += 1;
    }
    Err(format!("unterminated block comment at position {start}"))
}

fn starts_template(characters: &[char], index: usize) -> bool {
    characters.get(index) == Some(&'$') && characters.get(index + 1) == Some(&'{')
}

fn find_character(characters: &[char], needle: char, start: usize) -> Option<usize> {
    characters[start..]
        .iter()
        .position(|character| *character == needle)
        .map(|offset| start + offset)
}

fn is_symbol(character: char) -> bool {
    matches!(character, '(' | ')' | '[' | ']' | '{' | '}' | ',')
}
