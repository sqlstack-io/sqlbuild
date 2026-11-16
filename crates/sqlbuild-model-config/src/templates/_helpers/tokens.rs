//! The Python template tokenizer.

use crate::templates::_helpers::expressions::parse_expression;
use crate::templates::_helpers::pattern::{is_quote, template_body_length};
use crate::templates::errors::TemplateError;
use crate::templates::models::{TemplateFailure, TextPart};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum TokenKind {
    Word(String),
    Text(String),
    /// A quoted argument holding `${...}` templates, and its text as written.
    Interpolated {
        written: String,
        parts: Vec<TextPart>,
    },
    Symbol(char),
    End,
}

/// One token and its character position in the expression.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Token {
    pub(crate) kind: TokenKind,
    pub(crate) position: usize,
}

impl Token {
    /// Return the token's value as Python's error messages show it.
    pub(crate) fn value(&self) -> String {
        match &self.kind {
            TokenKind::Word(text)
            | TokenKind::Text(text)
            | TokenKind::Interpolated { written: text, .. } => text.clone(),
            TokenKind::Symbol(symbol) => symbol.to_string(),
            TokenKind::End => String::new(),
        }
    }
}

const SYMBOLS: [char; 3] = ['(', ')', ','];
const ESCAPE: char = '\\';

/// Split one template expression into tokens.
pub(crate) fn tokenize(expression: &str) -> Result<Vec<Token>, TemplateFailure> {
    let characters: Vec<char> = expression.chars().collect();
    let mut tokens: Vec<Token> = Vec::new();
    let mut index = 0;
    while let Some(&character) = characters.get(index) {
        if char::is_whitespace(character) {
            index += 1;
        } else if SYMBOLS.contains(&character) {
            tokens.push(Token {
                kind: TokenKind::Symbol(character),
                position: index,
            });
            index += 1;
        } else if is_quote(character) {
            let (kind, next) = quoted_text(&characters, index)?;
            tokens.push(Token {
                kind,
                position: index,
            });
            index = next;
        } else {
            let start = index;
            while let Some(&next) = characters.get(index) {
                if char::is_whitespace(next) || SYMBOLS.contains(&next) || is_quote(next) {
                    break;
                }
                index += 1;
            }
            tokens.push(Token {
                kind: TokenKind::Word(characters[start..index].iter().collect()),
                position: start,
            });
        }
    }
    tokens.push(Token {
        kind: TokenKind::End,
        position: characters.len(),
    });
    Ok(tokens)
}

fn quoted_text(characters: &[char], start: usize) -> Result<(TokenKind, usize), TemplateFailure> {
    let quote = characters[start];
    let mut parts: Vec<TextPart> = Vec::new();
    let mut text = String::new();
    let mut index = start + 1;
    while let Some(&character) = characters.get(index) {
        if character == ESCAPE {
            let Some(&escaped) = characters.get(index + 1) else {
                return Err(TemplateFailure::Invalid(
                    TemplateError::UnterminatedEscape { position: index },
                ));
            };
            text.push(escaped);
            index += 2;
        } else if character == quote {
            let kind = if parts.is_empty() {
                TokenKind::Text(text)
            } else {
                parts.push(TextPart::Text(text));
                TokenKind::Interpolated {
                    written: characters[start + 1..index].iter().collect(),
                    parts,
                }
            };
            return Ok((kind, index + 1));
        } else if let Some((body, length)) = nested_template(characters, index) {
            parts.push(TextPart::Text(std::mem::take(&mut text)));
            parts.push(TextPart::Template {
                expression: parse_expression(&body)?,
                body,
            });
            index += length;
        } else {
            text.push(character);
            index += 1;
        }
    }
    let quote = if quote == '\'' { "single" } else { "double" };
    Err(TemplateFailure::Invalid(
        TemplateError::UnterminatedString {
            quote,
            position: start,
        },
    ))
}

/// The body of a complete `${...}` template at `index`, and its length in characters.
fn nested_template(characters: &[char], index: usize) -> Option<(String, usize)> {
    if characters.get(index) != Some(&'$') || characters.get(index + 1) != Some(&'{') {
        return None;
    }
    let rest: String = characters[index + 2..].iter().collect();
    let length: usize = template_body_length(&rest)?;
    let body: String = rest[..length].to_owned();
    let count: usize = body.chars().count();
    Some((body, count + 3))
}
