//! The live rows of the `sqb dev` screen: the checklist before ready, the hotkey bar after.

use std::time::{Duration, Instant};

use sqlbuild_core::screen_text::main::render_dev_header::render_dev_header;
use sqlbuild_core::screen_text::main::render_frame_rule::render_frame_rule;
use sqlbuild_core::screen_text::main::render_hotkey_bar::render_hotkey_bar;
use sqlbuild_core::screen_text::main::render_phase_lines::render_phase_lines;
use sqlbuild_core::screen_text::models::{Hotkey, PhaseLayout, PhaseLine, PhaseState};

use crate::dev_screen::constants::{HOTKEYS, WAREHOUSE_FRAME_LABEL};
use crate::dev_screen::models::{DevScreen, DevStep, StepRow};

/// The label a checklist row shows for `step`.
pub(crate) fn step_label(step: DevStep) -> &'static str {
    match step {
        DevStep::ControlStore => "Control store",
        DevStep::Api => "API",
        DevStep::Scheduler => "Scheduler",
        DevStep::Discover => "Discover project",
        DevStep::Compile => "Compile snapshot",
        DevStep::Publish => "Publish",
        DevStep::Warehouse => "Warehouse",
    }
}

/// How long ago `started` was, if it was.
pub(crate) fn elapsed_since(started: Option<Instant>, now: Instant) -> Option<Duration> {
    started.map(|started| now.saturating_duration_since(started))
}

/// The rows redrawn in place: header and checklist, with the client's framed output under them.
pub(crate) fn live_rows(screen: &DevScreen, now: Instant) -> Vec<String> {
    if screen.ready {
        return vec![String::new(), render_hotkey_bar(&hotkeys(), &screen.facts)];
    }
    let lines: Vec<PhaseLine> = screen
        .steps
        .iter()
        .map(|row| phase_line(row, screen.frame, now))
        .collect();
    let mut rows: Vec<String> = vec![
        render_dev_header(&screen.header, &screen.facts),
        String::new(),
    ];
    rows.extend(render_phase_lines(
        &lines,
        PhaseLayout::TimeLast,
        &screen.facts,
    ));
    if screen.client_output {
        let label: &str = if screen.adapter.is_empty() {
            WAREHOUSE_FRAME_LABEL
        } else {
            &screen.adapter
        };
        rows.push(String::new());
        rows.push(render_frame_rule(Some(label), &screen.facts));
    }
    rows
}

/// One checklist row; a running row shows the current spinner frame and its live timer.
fn phase_line(row: &StepRow, frame: usize, now: Instant) -> PhaseLine {
    PhaseLine {
        state: match row.state {
            PhaseState::Running { .. } => PhaseState::Running { frame },
            state => state,
        },
        label: step_label(row.step).to_owned(),
        detail: row.detail.clone(),
        elapsed: row.elapsed.or_else(|| elapsed_since(row.started, now)),
    }
}

/// The hotkey bar's keys.
pub(crate) fn hotkeys() -> Vec<Hotkey> {
    HOTKEYS
        .iter()
        .map(|(key, label)| Hotkey {
            key: *key,
            label: (*label).to_owned(),
        })
        .collect()
}
