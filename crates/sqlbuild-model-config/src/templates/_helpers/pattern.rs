//! Where each `${...}` template in a string starts and ends.

const TEMPLATE_OPEN: &str = "${";
const ESCAPE: char = '\\';
/// How deep templates may nest inside quoted arguments; a deeper one does not close.
const MAX_NESTING: usize = 8;

/// Byte spans of each template, `${` to `}`, closing outside quotes or else at the first brace.
pub(crate) fn template_spans(text: &str) -> Vec<(usize, usize)> {
    counted_template_spans(text).0
}

/// `template_spans`, and how many characters the scan visited.
pub(crate) fn counted_template_spans(text: &str) -> (Vec<(usize, usize)>, usize) {
    let mut spans: Vec<(usize, usize)> = Vec::new();
    let mut steps: usize = 0;
    let mut index = 0;
    while let Some(offset) = text[index..].find(TEMPLATE_OPEN) {
        let start = index + offset;
        let body = start + TEMPLATE_OPEN.len();
        let (quoted, quoted_steps) = nested_body_length(&text[body..], 0);
        steps += quoted_steps;
        let length = match quoted {
            Some(length) => Some(length),
            None => {
                let (first_brace, brace_steps) = brace_free_length(&text[body..]);
                steps += brace_steps;
                first_brace
            }
        };
        if let Some(length) = length {
            spans.push((start, body + length + 1));
            index = body + length + 1;
        } else {
            index = start + 1;
        }
    }
    (spans, steps)
}

/// Byte length of the body at the start of `body` up to its `}`, skipping quoted arguments.
pub(crate) fn template_body_length(body: &str) -> Option<usize> {
    nested_body_length(body, 0).0
}

/// The quoted-argument-aware body length, if it closes, and the characters visited.
fn nested_body_length(body: &str, depth: usize) -> (Option<usize>, usize) {
    if depth > MAX_NESTING {
        return (None, 0);
    }
    let mut steps: usize = 0;
    let mut quote: Option<char> = None;
    let mut index = 0;
    while let Some(character) = body[index..].chars().next() {
        steps += 1;
        let mut next = index + character.len_utf8();
        match quote {
            Some(_) if character == ESCAPE => {
                next += body[next..].chars().next().map_or(0, char::len_utf8);
            }
            Some(open) if character == open => quote = None,
            Some(_) if body[index..].starts_with(TEMPLATE_OPEN) => {
                let nested = index + TEMPLATE_OPEN.len();
                let (length, nested_steps) = nested_body_length(&body[nested..], depth + 1);
                steps += nested_steps;
                let Some(length) = length else {
                    return (None, steps);
                };
                next = nested + length + 1;
            }
            Some(_) => {}
            None if is_quote(character) => quote = Some(character),
            None if character == '{' => return (None, steps),
            None if character == '}' => return ((index > 0).then_some(index), steps),
            None => {}
        }
        index = next;
    }
    (None, steps)
}

/// Python's rule, the body up to its first brace if that is a non-empty body's `}`, and the steps.
fn brace_free_length(body: &str) -> (Option<usize>, usize) {
    let Some(close) = body.find(['{', '}']) else {
        return (None, body.len());
    };
    (
        (close > 0 && body[close..].starts_with('}')).then_some(close),
        close + 1,
    )
}

/// Whether `character` opens a quoted argument.
pub(crate) fn is_quote(character: char) -> bool {
    matches!(character, '\'' | '"')
}
