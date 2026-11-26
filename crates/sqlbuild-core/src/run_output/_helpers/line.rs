//! A line built from styled pieces that knows its plain width, to fit it to the terminal.

use crate::cli_text::models::CliStyle;

/// The styled text and the plain text of one line, built together.
#[derive(Clone, Debug, Default)]
pub(crate) struct Line {
    plain: String,
    styled: String,
}

impl Line {
    /// Append `text`, styled by `paint`.
    pub(crate) fn push(&mut self, text: &str, paint: impl Fn(&str) -> String) -> &mut Self {
        self.plain.push_str(text);
        self.styled.push_str(&paint(text));
        self
    }

    /// Append already styled `text`, measured without its terminal styles.
    pub(crate) fn styled_piece(&mut self, text: &str) -> &mut Self {
        self.plain.push_str(&plain(text));
        self.styled.push_str(text);
        self
    }

    /// Append `text` unstyled.
    pub(crate) fn text(&mut self, text: &str) -> &mut Self {
        self.push(text, str::to_owned)
    }

    /// The plain width in characters.
    pub(crate) fn width(&self) -> usize {
        self.plain.chars().count()
    }

    /// The styled line when it fits `width`, else its plain text cut at the end and dimmed.
    pub(crate) fn fit(&self, width: usize, style: CliStyle) -> String {
        if self.width() <= width {
            return self.styled.clone();
        }
        let kept: String = self.plain.chars().take(width.saturating_sub(3)).collect();
        style.muted(&format!("{kept}..."))
    }
}

/// `text` without terminal styles.
pub(crate) fn plain(text: &str) -> String {
    let mut out: String = String::new();
    let mut escaping: bool = false;
    for character in text.chars() {
        match (escaping, character) {
            (false, '\u{1b}') => escaping = true,
            (true, 'm') => escaping = false,
            (true, _) => {}
            (false, _) => out.push(character),
        }
    }
    out
}
