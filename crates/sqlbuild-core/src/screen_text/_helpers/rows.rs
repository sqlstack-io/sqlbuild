//! Cut, pad and style the spans of one screen row so it never wraps.

use std::time::Duration;

use crate::screen_text::constants::{
    CUT_MARK, OSC8_END, OSC8_START, SECONDS_PER_HOUR, SECONDS_PER_MINUTE, TENTHS_PER_MINUTE,
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

/// A span of `text` in `tone` that links to `link`.
pub(crate) fn linked(text: impl Into<String>, tone: Tone, link: Option<String>) -> Segment {
    Segment {
        text: text.into(),
        tone,
        link,
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

/// `text` cut to `cells` with a trailing `…` when it is wider.
pub(crate) fn cut(text: &str, cells: usize) -> String {
    if cell_len(text) <= cells {
        return text.to_owned();
    }
    if cells == 0 {
        return String::new();
    }
    format!("{}{CUT_MARK}", take_cells(text, cells - 1))
}

/// `text` cut to `cells`, then padded with spaces to exactly `cells`.
pub(crate) fn column(text: &str, cells: usize) -> String {
    let kept: String = cut(text, cells);
    let padding: usize = cells.saturating_sub(cell_len(&kept));
    format!("{kept}{}", " ".repeat(padding))
}

/// `text` right-aligned in `cells`.
pub(crate) fn right(text: &str, cells: usize) -> String {
    format!("{}{text}", " ".repeat(cells.saturating_sub(cell_len(text))))
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
            "{}m {}s",
            seconds / SECONDS_PER_MINUTE,
            seconds % SECONDS_PER_MINUTE
        );
    }
    format!(
        "{}h {}m",
        seconds / SECONDS_PER_HOUR,
        (seconds % SECONDS_PER_HOUR) / SECONDS_PER_MINUTE
    )
}

/// The indented row of `segments`, without trailing spaces, cut with `…` to the screen width.
pub(crate) fn render_row(segments: Vec<Segment>, facts: &ScreenFacts) -> String {
    let segments: Vec<Segment> = without_trailing_space(segments);
    let budget: usize = facts.columns.saturating_sub(facts.indent);
    let total: usize = segments.iter().map(|segment| cell_len(&segment.text)).sum();
    let mut row: String = " ".repeat(facts.indent);
    if total <= budget {
        for segment in &segments {
            row.push_str(&styled(&segment.text, segment, facts));
        }
        return row;
    }
    let keep: usize = budget.saturating_sub(1);
    let mut used: usize = 0;
    for segment in &segments {
        if used >= keep {
            break;
        }
        let text: String = take_cells(&segment.text, keep - used);
        used += cell_len(&text);
        row.push_str(&styled(&text, segment, facts));
    }
    if budget > 0 {
        row.push_str(&facts.style.muted(CUT_MARK));
    }
    row
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
