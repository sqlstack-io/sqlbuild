//! Python's `_find_next_reference_start` walk over quoted text and comments.

use crate::compiler::_helpers::declaration_references::reference_syntax::{
    MatchedReference, ReferenceSyntax, match_reference,
};
use crate::compiler::models::{
    DeclarationReference, DeclarationReferenceKind, DeclarationReferenceScan,
    DeclarationReferenceStop,
};
use crate::sql_scan::main::dialect_non_code_end::dialect_non_code_end;
use crate::sql_scan::models::{LexicalSyntax, Unclosed};

const SPECIAL_BYTES: &[u8] = b"'\"`$@/-";
const ENUM_KEYWORD: &str = "@enum";
const CONSTANT_KEYWORD: &str = "@const";

/// Where the walk for the next reference start ended.
enum NextStart<'sql> {
    Reference(usize, MatchedReference<'sql>),
    /// A malformed reference start and the byte offset after its `@enum`/`@const` keyword.
    Malformed(DeclarationReferenceKind, usize),
    End,
    Stop(DeclarationReferenceStop),
}

/// Python's references in `sql` (code points) and stop; `skip_malformed` resumes past malformed.
pub(crate) fn scan_references(
    sql: &str,
    skip_malformed: bool,
    syntax: Option<&LexicalSyntax>,
) -> DeclarationReferenceScan {
    let mut references: Vec<DeclarationReference> = Vec::new();
    let mut offsets: CharOffsets = CharOffsets::default();
    let mut cursor: usize = 0;
    let mut stop: Option<DeclarationReferenceStop> = None;
    while cursor < sql.len() {
        let next: NextStart<'_> = match syntax {
            Some(syntax) => next_dialect_reference_start(sql, cursor, syntax),
            None => next_reference_start(sql, cursor),
        };
        let (start, matched) = match next {
            NextStart::Reference(start, matched) => (start, matched),
            NextStart::End => break,
            NextStart::Malformed(_, keyword_end) if skip_malformed => {
                cursor = keyword_end;
                continue;
            }
            NextStart::Malformed(kind, _) => {
                stop = Some(DeclarationReferenceStop::Malformed(kind));
                break;
            }
            NextStart::Stop(found) => {
                stop = Some(found);
                break;
            }
        };
        let start_char: usize = offsets.advance(sql, start);
        let end_char: usize = offsets.advance(sql, matched.end);
        references.push(DeclarationReference {
            kind: matched.kind,
            name: matched.name.to_owned(),
            member: matched.member.map(str::to_owned),
            start: start_char,
            end: end_char,
        });
        cursor = matched.end;
    }
    DeclarationReferenceScan { references, stop }
}

/// The next reference start at or after `start`, as Python's walk finds it.
fn next_reference_start(sql: &str, start: usize) -> NextStart<'_> {
    let rest: &str = &sql[start..];
    if !rest.contains("@enum") && !rest.contains("@const") {
        return NextStart::End;
    }
    let bytes: &[u8] = sql.as_bytes();
    let mut index: usize = start;
    while let Some(offset) = bytes[index..]
        .iter()
        .position(|byte| SPECIAL_BYTES.contains(byte))
    {
        index += offset;
        let next: Option<usize> = match bytes[index] {
            b'\'' | b'"' => quoted_end(bytes, index, true),
            b'`' => quoted_end(bytes, index, false),
            b'$' => dollar_end(bytes, index).map(|end| end.unwrap_or(index + 1)),
            b'-' if bytes.get(index + 1) == Some(&b'-') => Some(
                bytes[index..]
                    .iter()
                    .position(|byte| *byte == b'\n')
                    .map_or(bytes.len(), |newline| index + newline + 1),
            ),
            b'/' if bytes.get(index + 1) == Some(&b'*') => match find(&bytes[index + 2..], b"*/") {
                Some(close) => Some(index + 2 + close + 2),
                None => return NextStart::Stop(DeclarationReferenceStop::UnclosedBlockComment),
            },
            b'@' => match match_reference(sql, index) {
                ReferenceSyntax::NotReference => Some(index + 1),
                ReferenceSyntax::Matched(matched) => return NextStart::Reference(index, matched),
                ReferenceSyntax::Malformed(kind) => {
                    return NextStart::Malformed(kind, index + keyword_length(kind));
                }
            },
            _ => Some(index + 1),
        };
        match next {
            Some(next) => index = next,
            None => return NextStart::Stop(DeclarationReferenceStop::UnclosedQuote),
        }
    }
    NextStart::End
}

/// The next reference start at or after `start`, skipping text the dialect reads as non-code.
fn next_dialect_reference_start<'sql>(
    sql: &'sql str,
    start: usize,
    syntax: &LexicalSyntax,
) -> NextStart<'sql> {
    let rest: &str = &sql[start..];
    if !rest.contains("@enum") && !rest.contains("@const") {
        return NextStart::End;
    }
    let bytes: &[u8] = sql.as_bytes();
    let mut index: usize = start;
    while index < bytes.len() {
        if bytes[index] == b'@' {
            match match_reference(sql, index) {
                ReferenceSyntax::NotReference => index += 1,
                ReferenceSyntax::Matched(matched) => return NextStart::Reference(index, matched),
                ReferenceSyntax::Malformed(kind) => {
                    return NextStart::Malformed(kind, index + keyword_length(kind));
                }
            }
            continue;
        }
        index = match dialect_non_code_end(bytes, index, syntax) {
            Ok(Some(end)) => end,
            Ok(None) => index + 1,
            Err(Unclosed::BlockComment) => {
                return NextStart::Stop(DeclarationReferenceStop::UnclosedBlockComment);
            }
            Err(Unclosed::Quote | Unclosed::Parenthesis) => {
                return NextStart::Stop(DeclarationReferenceStop::UnclosedQuote);
            }
        };
    }
    NextStart::End
}

fn keyword_length(kind: DeclarationReferenceKind) -> usize {
    match kind {
        DeclarationReferenceKind::Enum => ENUM_KEYWORD.len(),
        DeclarationReferenceKind::Constant => CONSTANT_KEYWORD.len(),
    }
}

/// Python's `quoted_text_end_impl` for `'`, `"` (doubled quotes escape) and backticks.
fn quoted_end(bytes: &[u8], start: usize, doubled_escapes: bool) -> Option<usize> {
    let quote: u8 = bytes[start];
    let mut index: usize = start + 1;
    loop {
        index += bytes[index..].iter().position(|byte| *byte == quote)?;
        if doubled_escapes && bytes.get(index + 1) == Some(&quote) {
            index += 2;
            continue;
        }
        return Some(index + 1);
    }
}

/// Python's `_dollar_quoted_text_end`: Some(None) when `$` opens no literal.
fn dollar_end(bytes: &[u8], start: usize) -> Option<Option<usize>> {
    if start
        .checked_sub(1)
        .is_some_and(|previous| continues_dollar_word(bytes[previous]))
    {
        return Some(None);
    }
    let tag_end: usize = bytes[start + 1..]
        .iter()
        .position(|byte| !(byte.is_ascii_alphanumeric() || *byte == b'_'))
        .map_or(bytes.len(), |offset| start + 1 + offset);
    if bytes.get(tag_end) != Some(&b'$') || bytes.get(start + 1).is_some_and(u8::is_ascii_digit) {
        return Some(None);
    }
    let delimiter: &[u8] = &bytes[start..=tag_end];
    let closing: usize = find(&bytes[tag_end + 1..], delimiter)?;
    Some(Some(tag_end + 1 + closing + delimiter.len()))
}

fn continues_dollar_word(byte: u8) -> bool {
    !byte.is_ascii() || byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'$')
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

/// Converts increasing byte offsets into Python code-point offsets.
#[derive(Default)]
struct CharOffsets {
    byte: usize,
    char: usize,
}

impl CharOffsets {
    fn advance(&mut self, sql: &str, byte: usize) -> usize {
        self.char += sql[self.byte..byte].chars().count();
        self.byte = byte;
        self.char
    }
}
