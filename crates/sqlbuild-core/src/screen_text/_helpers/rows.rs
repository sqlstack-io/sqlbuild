//! Cut, wrap and style the spans of screen rows.

use std::time::Duration;

use crate::screen_text::constants::{
    CUT_MARK, MIN_WRAP, OSC8_END, OSC8_START, SECONDS_PER_HOUR, SECONDS_PER_MINUTE,
    TENTHS_PER_MINUTE,
};
use crate::screen_text::models::{ScreenFacts, Segment, Tone};
use crate::text::main::cell_len::cell_len;
use crate::text::main::char_cells::char_cells;

/// A span of `text` in `tone`.
pub(crate) fn span(text: impl Into<String>, tone: Tone) -> Segment {
    Segment {
        text: text.into(),
        tone,
        link: None,
    }
}

/// The leading characters of `text` that fit in `cells`.
pub(crate) fn take_cells(text: &str, cells: usize) -> String {
    let mut used: usize = 0;
    let mut kept: String = String::new();
    for character in text.chars() {
        let width: usize = char_cells(character);
        if used + width > cells {
            break;
        }
        used += width;
        kept.push(character);
    }
    kept
}

/// `text` right-aligned in `cells`.
pub(crate) fn right(text: &str, cells: usize) -> String {
    format!("{}{text}", " ".repeat(cells.saturating_sub(cell_len(text))))
}

/// `text` padded with spaces to at least `cells`.
pub(crate) fn pad(text: &str, cells: usize) -> String {
    format!("{text}{}", " ".repeat(cells.saturating_sub(cell_len(text))))
}

/// An elapsed time as `0.1s`, `16m 43s` or `2h 5m`, truncated rather than rounded.
pub(crate) fn elapsed_text(elapsed: Duration) -> String {
    let tenths: u128 = elapsed.as_millis() / 100;
    if tenths < TENTHS_PER_MINUTE {
        return format!("{}.{}s", tenths / 10, tenths % 10);
    }
    let seconds: u64 = elapsed.as_secs();
    if seconds < SECONDS_PER_HOUR {
        return format!(
            "{}m {:02}s",
            seconds / SECONDS_PER_MINUTE,
            seconds % SECONDS_PER_MINUTE
        );
    }
    format!(
        "{}h {:02}m",
        seconds / SECONDS_PER_HOUR,
        (seconds % SECONDS_PER_HOUR) / SECONDS_PER_MINUTE
    )
}

/// The widest a row may be: one cell short of the terminal, so the cursor never wraps.
pub(crate) fn row_width(facts: &ScreenFacts) -> usize {
    facts.columns.saturating_sub(1).max(1)
}

/// The room lasting text has after `column`, never less than a readable minimum.
pub(crate) fn wrap_width(facts: &ScreenFacts, column: usize) -> usize {
    row_width(facts).saturating_sub(column).max(MIN_WRAP)
}

/// `segments` as one row, without trailing spaces, cut with `…` to `width` cells.
pub(crate) fn fit_row(segments: Vec<Segment>, width: usize, facts: &ScreenFacts) -> String {
    let segments: Vec<Segment> = without_trailing_space(segments);
    let total: usize = segments.iter().map(|segment| cell_len(&segment.text)).sum();
    let mut row: String = String::new();
    if total <= width {
        for segment in &segments {
            row.push_str(&styled(&segment.text, segment, facts));
        }
        return row;
    }
    let keep: usize = width.saturating_sub(1);
    let mut used: usize = 0;
    for segment in &segments {
        if used >= keep {
            break;
        }
        let text: String = take_cells(&segment.text, keep - used);
        used += cell_len(&text);
        row.push_str(&styled(&text, segment, facts));
    }
    if width > 0 {
        row.push_str(&facts.style.muted(CUT_MARK));
    }
    row
}

/// `segments` styled, uncut.
pub(crate) fn paint(segments: &[Segment], facts: &ScreenFacts) -> String {
    let segments: Vec<Segment> = without_trailing_space(segments.to_vec());
    segments
        .iter()
        .map(|segment| styled(&segment.text, segment, facts))
        .collect()
}

/// `text` wrapped at word boundaries into lines of at most `width` cells; longer words are broken.
pub(crate) fn wrap_words(text: &str, width: usize) -> Vec<String> {
    let width: usize = width.max(1);
    let mut lines: Vec<String> = Vec::new();
    for paragraph in text.split('\n') {
        let mut line: String = String::new();
        for word in paragraph.trim_end().split(' ') {
            if word.is_empty() && line.is_empty() {
                continue;
            }
            let mut word: String = word.to_owned();
            loop {
                let room: usize = if line.is_empty() {
                    width
                } else {
                    width.saturating_sub(cell_len(&line) + 1)
                };
                if cell_len(&word) <= room {
                    if !line.is_empty() {
                        line.push(' ');
                    }
                    line.push_str(&word);
                    break;
                }
                if !line.is_empty() {
                    lines.push(std::mem::take(&mut line).trim_end().to_owned());
                    if word.is_empty() {
                        break;
                    }
                    continue;
                }
                let head: String = leading_cells(&word, width);
                word = word[head.len()..].to_owned();
                lines.push(head);
                if word.is_empty() {
                    break;
                }
            }
        }
        lines.push(line);
    }
    while lines.len() > 1 && lines.last().is_some_and(String::is_empty) {
        lines.pop();
    }
    lines
}

/// `text` broken into rows of at most `width` cells, keeping its spacing; control characters are dropped.
pub(crate) fn break_cells(text: &str, width: usize) -> Vec<String> {
    let width: usize = width.max(1);
    let mut rows: Vec<String> = Vec::new();
    for line in text.split('\n') {
        let mut rest: String = line
            .chars()
            .filter(|character| !character.is_control())
            .collect();
        loop {
            let head: String = leading_cells(&rest, width);
            let done: bool = head.len() == rest.len();
            rest = rest[head.len()..].to_owned();
            rows.push(head);
            if done || rest.is_empty() {
                break;
            }
        }
    }
    rows
}

/// The leading characters of `text` that fit in `cells`, and at least one so wrapping always advances.
fn leading_cells(text: &str, cells: usize) -> String {
    let head: String = take_cells(text, cells);
    if head.is_empty() {
        return text.chars().take(1).collect();
    }
    head
}

fn without_trailing_space(mut segments: Vec<Segment>) -> Vec<Segment> {
    while segments
        .last()
        .is_some_and(|segment| segment.text.trim_end().is_empty())
    {
        segments.pop();
    }
    if let Some(last) = segments.last_mut() {
        last.text.truncate(last.text.trim_end().len());
    }
    segments
}

fn styled(text: &str, segment: &Segment, facts: &ScreenFacts) -> String {
    let style = facts.style;
    let painted: String = match segment.tone {
        Tone::Plain => text.to_owned(),
        Tone::Muted => style.muted(text),
        Tone::Strong => style.section(text),
        Tone::Success => style.success(text),
        Tone::Warning => style.warning(text),
        Tone::Error => style.error(text),
        Tone::Change => style.value(text),
        Tone::Link => style.link(text),
    };
    match &segment.link {
        Some(target) if facts.links && !text.is_empty() => {
            format!("{OSC8_START}{target}{OSC8_END}{painted}{OSC8_START}{OSC8_END}")
        }
        _ => painted,
    }
}
