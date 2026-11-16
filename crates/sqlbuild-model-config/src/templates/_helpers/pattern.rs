//! Where each `${...}` template in a string starts and ends.

const TEMPLATE_OPEN: &str = "${";
const ESCAPE: char = '\\';
/// How deep templates may nest inside quoted arguments; a deeper one does not close.
const MAX_NESTING: usize = 8;

/// Return the byte spans of every template match, including the `${` and `}`.
///
/// A template ends at the first `}` outside its quoted arguments, which may hold braces and whole
/// templates. A body that never closes that way, such as one with an unbalanced quote or an
/// unclosed nested template, ends at its first brace as Python's `\$\{([^{}]+)\}` does, so a
/// malformed body still reports its error.
pub(crate) fn template_spans(text: &str) -> Vec<(usize, usize)> {
    let mut spans: Vec<(usize, usize)> = Vec::new();
    let mut index = 0;
    while let Some(offset) = text[index..].find(TEMPLATE_OPEN) {
        let start = index + offset;
        let body = start + TEMPLATE_OPEN.len();
        let length =
            template_body_length(&text[body..]).or_else(|| brace_free_length(&text[body..]));
        if let Some(length) = length {
            spans.push((start, body + length + 1));
            index = body + length + 1;
        } else {
            index = start + 1;
        }
    }
    spans
}

/// The byte length of the non-empty template body at the start of `body`, up to its closing `}`.
///
/// Quoted arguments may hold braces and whole `${...}` templates, and a backslash inside them
/// escapes the next character. A `{` outside them, a nested template that does not close, or no
/// closing `}` gives `None`. Each character is scanned once per nesting level.
pub(crate) fn template_body_length(body: &str) -> Option<usize> {
    nested_body_length(body, 0)
}

fn nested_body_length(body: &str, depth: usize) -> Option<usize> {
    if depth > MAX_NESTING {
        return None;
    }
    let mut quote: Option<char> = None;
    let mut index = 0;
    while let Some(character) = body[index..].chars().next() {
        let mut next = index + character.len_utf8();
        match quote {
            Some(_) if character == ESCAPE => {
                next += body[next..].chars().next().map_or(0, char::len_utf8);
            }
            Some(open) if character == open => quote = None,
            Some(_) if body[index..].starts_with(TEMPLATE_OPEN) => {
                let nested = index + TEMPLATE_OPEN.len();
                next = nested + nested_body_length(&body[nested..], depth + 1)? + 1;
            }
            Some(_) => {}
            None if is_quote(character) => quote = Some(character),
            None if character == '{' => return None,
            None if character == '}' => return (index > 0).then_some(index),
            None => {}
        }
        index = next;
    }
    None
}

/// Python's rule: the body runs to the first brace, which must be a non-empty body's `}`.
fn brace_free_length(body: &str) -> Option<usize> {
    let close = body.find(['{', '}'])?;
    (close > 0 && body[close..].starts_with('}')).then_some(close)
}

/// Whether `character` opens a quoted argument.
pub(crate) fn is_quote(character: char) -> bool {
    matches!(character, '\'' | '"')
}
