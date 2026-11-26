//! The live footer and the grouped completed rows of `sqb test` and `sqb scenario test`.

use std::time::Instant;

use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyResult};
use sqlbuild_core::build_progress::main::heartbeat_interval::heartbeat_interval;
use sqlbuild_core::nested_progress::models::NestedChildRow;
use sqlbuild_core::run_output::models::{ProgressFooter, RunCounts};

use crate::build_progress::_helpers::result_text::text;
use sqlbuild_python_base::boundary::models::KeywordArguments;

pub(crate) const MIN_NAME_WIDTH: usize = 20;
pub(crate) const PASS_STATUS: &str = "PASS";
pub(crate) const TIMEOUT_STATUS: &str = "TIMEOUT";
/// Clear the current line and go to its start.
pub(crate) const CLEAR_FOOTER_LINE: &str = "\r\x1b[K";
/// Go up a line and clear it.
pub(crate) const FOOTER_LINE_UP: &str = "\x1b[1A\x1b[K";
pub(crate) const ITEM_OPTIONS: (&[&str], &[&str]) = (
    &["group_name", "item_name"],
    &[
        "status_text",
        "run_on",
        "detail",
        "not_run",
        "error_code",
        "error_help",
        "error_message",
        "child_rows",
        "canonical_resource_name",
        "canonical_resource_id",
    ],
);

/// One started item: its name, group, when it started and whether it has finished.
pub(crate) struct RunningItem {
    pub(crate) name: String,
    pub(crate) group: String,
    pub(crate) started: Instant,
    /// Finished executing; its row waits for an earlier item's result.
    pub(crate) finished: bool,
}

/// Mutable reporter state; only held briefly and never across Python calls.
#[derive(Default)]
pub(crate) struct ReporterState {
    pub(crate) printed_group: Option<String>,
    pub(crate) running: Vec<RunningItem>,
    pub(crate) counts: RunCounts,
    pub(crate) frame: usize,
    pub(crate) line_active: bool,
    /// How many footer lines are on the terminal now.
    pub(crate) drawn_lines: usize,
    pub(crate) cursor_hidden: bool,
    pub(crate) stop_event: Option<Py<PyAny>>,
    pub(crate) thread: Option<Py<PyAny>>,
    /// Whether item progress waits for each item's result, which may abandon the attempt.
    pub(crate) holding: bool,
    pub(crate) started: Option<Instant>,
    /// Heartbeats written so far, and when the last one was.
    pub(crate) beats: usize,
    pub(crate) last_beat: Option<Instant>,
}

impl ReporterState {
    /// The footer for the items now running, held back and done.
    pub(crate) fn footer(&self, (verb, noun): (&str, &str)) -> ProgressFooter {
        let now: Instant = Instant::now();
        let oldest: Option<&RunningItem> = self
            .running
            .iter()
            .filter(|item| !item.finished)
            .min_by_key(|item| item.started);
        ProgressFooter {
            verb: verb.to_owned(),
            noun: noun.to_owned(),
            counts: self.counts,
            running: self.running.iter().filter(|item| !item.finished).count(),
            held: self.running.iter().filter(|item| item.finished).count(),
            oldest: oldest.map(|item| {
                (
                    item.name.clone(),
                    now.duration_since(item.started).as_secs_f64(),
                )
            }),
            elapsed: self
                .started
                .map_or(0.0, |started| now.duration_since(started).as_secs_f64()),
        }
    }

    /// Whether a heartbeat is due: the first after 30 s, then 1 min, 2 min and every 5 min.
    pub(crate) fn heartbeat_due(&self) -> bool {
        let Some(since) = self.last_beat.or(self.started) else {
            return false;
        };
        !self.running.is_empty() && since.elapsed() >= heartbeat_interval(self.beats)
    }
}

/// The text that erases `lines` footer lines and leaves the cursor where the first one began.
pub(crate) fn clear_footer(lines: usize) -> String {
    let mut text: String = CLEAR_FOOTER_LINE.to_owned();
    for _ in 1..lines.max(1) {
        text.push_str(FOOTER_LINE_UP);
    }
    text
}

pub(crate) fn optional_argument(
    arguments: &KeywordArguments<'_>,
    name: &str,
) -> PyResult<Option<String>> {
    if !arguments.passed(name)? {
        return Ok(None);
    }
    let value: Bound<'_, PyAny> = arguments.value(name)?;
    if value.is_none() {
        return Ok(None);
    }
    value.extract().map(Some)
}

pub(crate) fn child_rows(arguments: &KeywordArguments<'_>) -> PyResult<Vec<NestedChildRow>> {
    if !arguments.passed("child_rows")? {
        return Ok(Vec::new());
    }
    let mut rows: Vec<NestedChildRow> = Vec::new();
    for row in arguments.value("child_rows")?.try_iter()? {
        let row: Bound<'_, PyAny> = row?;
        rows.push(NestedChildRow {
            label: text(&row, "label")?,
            name: text(&row, "name")?,
            status_text: text(&row, "status_text")?,
            detail: text(&row, "detail")?,
        });
    }
    Ok(rows)
}
