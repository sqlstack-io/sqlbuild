//! The `${CTX:key}` and `${name}` placeholders of a deferred target's schema or database.

/// The deferred-target context marker inside `${...}`.
pub(crate) const CONTEXT_PREFIX: &str = "CTX:";
/// The opening of a `${...}` placeholder.
pub(crate) const PLACEHOLDER_OPEN: &str = "${";
const PLACEHOLDER_CLOSE: char = '}';

/// Each `{open}body}` in `text` whose body is non-empty and free of `}` and `forbidden`, in order.
pub(crate) fn placeholders<'t>(
    text: &'t str,
    open: &str,
    forbidden: Option<char>,
) -> Vec<(usize, usize, &'t str)> {
    let mut found: Vec<(usize, usize, &str)> = Vec::new();
    let mut from = 0;
    while let Some(offset) = text[from..].find(open) {
        let start = from + offset;
        let body_start = start + open.len();
        let body_length = text[body_start..]
            .find(|character: char| character == PLACEHOLDER_CLOSE || Some(character) == forbidden)
            .unwrap_or(text.len() - body_start);
        let close = body_start + body_length;
        if body_length > 0 && text[close..].starts_with(PLACEHOLDER_CLOSE) {
            found.push((start, close + 1, &text[body_start..close]));
            from = close + 1;
        } else {
            from = start + 1;
        }
    }
    found
}

/// `text` with each `${CTX:key}` naming a key in `keys` replaced by `logical`.
pub(crate) fn substitute_context(text: &str, keys: &[String], logical: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut copied = 0;
    let open = format!("{PLACEHOLDER_OPEN}{CONTEXT_PREFIX}");
    for (start, end, key) in placeholders(text, &open, None) {
        if keys.iter().any(|known| known == key) {
            result.push_str(&text[copied..start]);
            result.push_str(logical);
            copied = end;
        }
    }
    result.push_str(&text[copied..]);
    result
}
