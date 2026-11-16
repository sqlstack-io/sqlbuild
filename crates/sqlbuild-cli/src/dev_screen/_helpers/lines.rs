//! The lines the `sqb dev` screen prints: event lines, the ready card, and the redraw that places them.

use std::time::{Duration, Instant};

use sqlbuild_core::screen_text::main::format_plain_line::format_plain_line;
use sqlbuild_core::screen_text::main::redraw_rows::redraw_rows;
use sqlbuild_core::screen_text::main::render_event_line::render_event_line;
use sqlbuild_core::screen_text::main::render_ready_card::render_ready_card;
use sqlbuild_core::screen_text::models::{EventLine, EventMark, ReadyCard, Segment};

use crate::dev_screen::_helpers::rows::live_rows;
use crate::dev_screen::models::{DevScreen, DevStep};

/// One event: its live line, and the plain line's message.
pub(crate) struct Report<'a> {
    pub(crate) clock: &'a str,
    pub(crate) mark: EventMark,
    pub(crate) title: String,
    pub(crate) detail: String,
    pub(crate) elapsed: Option<Duration>,
    pub(crate) notes: Vec<Vec<Segment>>,
    pub(crate) plain: String,
}

/// The event's rows on a live screen, or its one plain line.
pub(crate) fn report_lines(screen: &DevScreen, report: Report<'_>) -> Vec<String> {
    if !screen.live {
        return vec![format_plain_line(&report.plain, report.elapsed)];
    }
    render_event_line(
        &EventLine {
            clock: report.clock.to_owned(),
            mark: report.mark,
            title: report.title,
            detail: report.detail,
            elapsed: report.elapsed,
            notes: report.notes,
        },
        &screen.facts,
    )
}

/// The card, or the plain `ready` line, once the first generation is published and the warehouse settled.
pub(crate) fn ready_lines(mut screen: DevScreen, now: Instant) -> (DevScreen, Vec<String>) {
    let published: bool = screen
        .steps
        .iter()
        .any(|row| row.step == DevStep::Publish && row.elapsed.is_some());
    let (Some(generation), Some(warehouse), false, true) = (
        screen.serving,
        screen.warehouse.clone(),
        screen.ready,
        published,
    ) else {
        return (screen, Vec::new());
    };
    screen.ready = true;
    if !screen.live {
        let line: String = format_plain_line(&format!("ready {}", screen.ui_url), None);
        return (screen, vec![line]);
    }
    let mut location: String = format!("{} \u{b7} gen {generation}", screen.header.target);
    if !screen.compile_detail.is_empty() {
        location = format!("{location} \u{b7} {}", screen.compile_detail);
    }
    let lines: Vec<String> = render_ready_card(
        &ReadyCard {
            version: screen.header.version.clone(),
            ready_in: now.saturating_duration_since(screen.started),
            ui_url: screen.ui_url.clone(),
            location,
            adapter: screen.adapter.clone(),
            warehouse,
            triggers: screen.triggers.clone(),
            watching: screen.watching.clone(),
        },
        &screen.facts,
    );
    screen.event_gap = true;
    (screen, lines)
}

/// `raw` client text, then `lines` printed for good, then the live rows redrawn below them.
pub(crate) fn compose(
    mut screen: DevScreen,
    (raw, lines): (&str, &[String]),
    now: Instant,
) -> (DevScreen, String) {
    let mut text: String = raw.to_owned();
    if !screen.live {
        for line in lines {
            text.push_str(line);
            text.push('\n');
        }
        return (screen, text);
    }
    if screen.client_output {
        return (screen, text);
    }
    let live: Vec<String> = live_rows(&screen, now);
    let mut rows: Vec<String> = lines.to_vec();
    rows.extend(live.iter().cloned());
    text.push_str(&redraw_rows(screen.drawn, &rows));
    screen.drawn = live.len();
    (screen, text)
}
