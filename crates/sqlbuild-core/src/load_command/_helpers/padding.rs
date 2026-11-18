//! Python format-spec padding and digit grouping.

/// `f"{text:<width}"`: left-aligned, padded by character count.
pub(crate) fn pad_right(text: &str, width: usize) -> String {
    let length: usize = text.chars().count();
    format!("{text}{}", " ".repeat(width.saturating_sub(length)))
}
