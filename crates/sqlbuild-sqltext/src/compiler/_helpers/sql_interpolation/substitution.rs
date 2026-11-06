//! `@@name`, `@@ENV:NAME` and `@@CTX:name` interpolation of authored SQL.

use crate::compiler::models::CharSpan;
use crate::compiler::models::{InterpolatedSql, InterpolationFailure, InterpolationRead};
use crate::compiler::types::InterpolationHost;
use crate::sql_scan::main::dialect_non_code_end::dialect_non_code_end;
use crate::sql_scan::main::non_code_end::non_code_end;
use crate::sql_scan::models::{QuotePolicy, Unclosed};
use sqlbuild_core::text::main::is_letter::is_letter;

const CONTEXT: &str = "SQL interpolation";
const TOKEN: &str = "@@";
const ENVIRONMENT_PREFIX: &str = "ENV:";
const CONTEXT_PREFIX: &str = "CTX:";

/// One substitution in bytes: source start and end, output start and end.
struct ByteSpan {
    source_start: usize,
    source_end: usize,
    output_start: usize,
    output_end: usize,
}

struct Interpolation<'run, H: InterpolationHost> {
    host: &'run H,
    file_path: &'run str,
    output: String,
    spans: Vec<ByteSpan>,
    reads: Vec<InterpolationRead>,
}

/// Interpolate `sql` as Python's `substitute_sql_vars_with_spans` did, or return its error.
pub(crate) fn interpolate<H: InterpolationHost>(
    host: &H,
    sql: &str,
    file_path: &str,
) -> Result<InterpolatedSql, InterpolationFailure> {
    let mut run = Interpolation {
        host,
        file_path,
        output: String::new(),
        spans: Vec::new(),
        reads: Vec::new(),
    };
    if !sql.contains(TOKEN) {
        return Ok(InterpolatedSql {
            sql: None,
            spans: Vec::new(),
            reads: Vec::new(),
        });
    }
    match run.scan(sql) {
        Ok(()) => {
            let spans = character_spans(sql, &run.output, &run.spans);
            let changed = run.output != sql;
            Ok(InterpolatedSql {
                sql: changed.then_some(run.output),
                spans,
                reads: run.reads,
            })
        }
        Err(message) => Err(InterpolationFailure {
            message,
            reads: run.reads,
        }),
    }
}

impl<H: InterpolationHost> Interpolation<'_, H> {
    fn scan(&mut self, sql: &str) -> Result<(), String> {
        let bytes = sql.as_bytes();
        let mut index = 0;
        while index < bytes.len() {
            let non_code = match self.host.lexical_syntax() {
                Some(syntax) if bytes[index] != b'`' => dialect_non_code_end(bytes, index, syntax),
                _ => python_non_code_end(bytes, index),
            };
            let text_end = match non_code {
                Ok(Some(end)) => end,
                Ok(None) => index,
                Err(Unclosed::BlockComment) => {
                    return Err(format!("{CONTEXT} contains an unclosed block comment"));
                }
                Err(Unclosed::Quote | Unclosed::Parenthesis) => {
                    return Err(format!("{CONTEXT} contains an unclosed quoted string"));
                }
            };
            if text_end > index {
                let comment = !matches!(bytes[index], b'\'' | b'"' | b'`' | b'$');
                if comment {
                    self.output.push_str(&sql[index..text_end]);
                } else {
                    self.quoted_text(sql, index, text_end)?;
                }
                index = text_end;
                continue;
            }
            if bytes[index..].starts_with(TOKEN.as_bytes()) {
                index = self.token(sql, index, bytes.len())?;
                continue;
            }
            let character = sql[index..].chars().next().unwrap_or_default();
            self.output.push(character);
            index += character.len_utf8();
        }
        Ok(())
    }

    /// Python's `_interpolate_sql_segment`: every token inside one quoted literal.
    fn quoted_text(&mut self, sql: &str, start: usize, end: usize) -> Result<(), String> {
        let mut index = start;
        while let Some(offset) = sql[index..end].find(TOKEN) {
            self.output.push_str(&sql[index..index + offset]);
            index = self.token(sql, index + offset, end)?;
        }
        self.output.push_str(&sql[index..end]);
        Ok(())
    }

    /// Render the token at `start`, which may not read past `limit`; return where it ends.
    fn token(&mut self, sql: &str, start: usize, limit: usize) -> Result<usize, String> {
        let text = &sql[..limit];
        let output_start = self.output.len();
        let end = self.rendered_token(text, start)?;
        self.spans.push(ByteSpan {
            source_start: start,
            source_end: end,
            output_start,
            output_end: self.output.len(),
        });
        Ok(end)
    }

    fn rendered_token(&mut self, sql: &str, start: usize) -> Result<usize, String> {
        let file_path = self.file_path;
        if sql[start..].starts_with("@@@") {
            let name_start = start + 3;
            let end = if self.starts_identifier(sql, name_start) {
                self.identifier_end(sql, name_start)
            } else {
                name_start
            };
            self.output.push_str(&sql[start..end]);
            return Ok(end);
        }
        let token_start = start + TOKEN.len();
        if sql[token_start..].starts_with(ENVIRONMENT_PREFIX) {
            let name_start = token_start + ENVIRONMENT_PREFIX.len();
            let name_end = self.name_end(sql, name_start, &['_']);
            if name_end == name_start {
                return Err(format!(
                    "invalid environment interpolation token in '{file_path}'"
                ));
            }
            let name = &sql[name_start..name_end];
            self.reads
                .push(InterpolationRead::Environment(name.to_owned()));
            let value = self.host.environment(name)?.ok_or_else(|| {
                format!("unknown environment variable '@@ENV:{name}' in '{file_path}'")
            })?;
            self.output.push_str(&value);
            return Ok(name_end);
        }
        if sql[token_start..].starts_with(CONTEXT_PREFIX) {
            let name_start = token_start + CONTEXT_PREFIX.len();
            let greedy_end = self.name_end(sql, name_start, &['_', '.']);
            if greedy_end == name_start {
                return Err(format!("invalid CTX interpolation token in '{file_path}'"));
            }
            if !self.host.context_allowed() {
                return Err(format!(
                    "SQL text in '{file_path}' does not allow @@CTX templates"
                ));
            }
            let name_end = self.known_context_end(sql, name_start, greedy_end);
            let name = &sql[name_start..name_end];
            self.reads.push(InterpolationRead::Context(name.to_owned()));
            let value = self.host.context(name).ok_or_else(|| {
                format!("SQL text in '{file_path}' references unknown CTX key '{name}'")
            })?;
            let value = value.ok_or_else(|| {
                format!(
                    "SQL text in '{file_path}' references CTX key '{name}' but no value is \
                     available"
                )
            })?;
            self.output.push_str(&value);
            return Ok(name_end);
        }
        if self.starts_identifier(sql, token_start) {
            let name_end = self.identifier_end(sql, token_start);
            let name = &sql[token_start..name_end];
            let value = self.host.variable(name).ok_or_else(|| {
                let names = self.host.variable_names();
                let available = if names.is_empty() {
                    "none".to_owned()
                } else {
                    names.join(", ")
                };
                format!(
                    "unknown project variable '@@{name}' in '{file_path}'. Available vars: \
                     {available}"
                )
            })??;
            self.output.push_str(&value);
            return Ok(name_end);
        }
        self.output.push_str(TOKEN);
        Ok(token_start)
    }

    fn starts_identifier(&self, sql: &str, at: usize) -> bool {
        sql[at..]
            .chars()
            .next()
            .is_some_and(|character| character == '_' || is_letter(character))
    }

    /// The end of the identifier starting at `start`, whose first character already matched.
    fn identifier_end(&self, sql: &str, start: usize) -> usize {
        let first = sql[start..].chars().next().map_or(0, char::len_utf8);
        self.name_end(sql, start + first, &['_'])
    }

    /// The end of the run of Python `str.isalnum()` characters and `extra` from `start`.
    fn name_end(&self, sql: &str, start: usize, extra: &[char]) -> usize {
        sql[start..]
            .char_indices()
            .find(|(_, character)| {
                !(extra.contains(character) || char::is_alphanumeric(*character))
            })
            .map_or(sql.len(), |(offset, _)| start + offset)
    }

    /// Python's `_known_context_name_end`: the longest known name before a `.` in the greedy one.
    fn known_context_end(&self, sql: &str, start: usize, greedy_end: usize) -> usize {
        let greedy = &sql[start..greedy_end];
        if self.host.context(greedy).is_some() {
            return greedy_end;
        }
        self.host
            .context_names()
            .into_iter()
            .filter(|name| {
                greedy
                    .strip_prefix(name.as_str())
                    .is_some_and(|rest| rest.starts_with('.'))
            })
            .map(|name| name.len())
            .max()
            .map_or(greedy_end, |length| start + length)
    }
}

/// Python's comment and quote end at `index`; only `'` and `"` double, so backticks never escape.
fn python_non_code_end(bytes: &[u8], index: usize) -> Result<Option<usize>, Unclosed> {
    if bytes[index] != b'`' {
        return non_code_end(bytes, index, QuotePolicy::COMPILER);
    }
    bytes[index + 1..]
        .iter()
        .position(|byte| *byte == b'`')
        .map(|offset| Some(index + 1 + offset + 1))
        .ok_or(Unclosed::Quote)
}

/// Convert byte spans to the code-point spans Python's `ExpansionSpan` holds.
fn character_spans(source: &str, output: &str, spans: &[ByteSpan]) -> Vec<CharSpan> {
    let source_chars = char_offsets(source);
    let output_chars = char_offsets(output);
    spans
        .iter()
        .map(|span| CharSpan {
            source_start: source_chars[span.source_start],
            source_end: source_chars[span.source_end],
            output_start: output_chars[span.output_start],
            output_end: output_chars[span.output_end],
        })
        .collect()
}

/// The code-point offset of every byte boundary of `text`, indexed by byte offset.
fn char_offsets(text: &str) -> Vec<usize> {
    let mut offsets = vec![0; text.len() + 1];
    let mut count = 0;
    for (byte, character) in text.char_indices() {
        for slot in &mut offsets[byte..byte + character.len_utf8()] {
            *slot = count;
        }
        count += 1;
    }
    offsets[text.len()] = count;
    offsets
}
