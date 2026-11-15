//! How each message changes the `sqb dev` screen, and the lines it prints for good.

use std::time::{Duration, Instant};

use sqlbuild_core::screen_text::main::format_plain_line::format_plain_line;
use sqlbuild_core::screen_text::main::redraw_rows::redraw_rows;
use sqlbuild_core::screen_text::main::render_frame_rule::render_frame_rule;
use sqlbuild_core::screen_text::main::render_log_line::render_log_line;
use sqlbuild_core::screen_text::models::{EventMark, PhaseState, Segment, Tone, WarehouseState};
use sqlbuild_orchestration::dev_events::models::{
    DevEvent, DevPhase, DevStepState, SourcePosition,
};

use crate::dev_screen::_helpers::lines::{Report, report_lines, step_row};
use crate::dev_screen::_helpers::rows::live_rows;
use crate::dev_screen::constants::{FAILED_HINT, LOG_RING_LINES, SNAPSHOT_ID_CHARS};
use crate::dev_screen::models::{DevScreen, DevStep, ScreenAction};

/// A startup step began; the plain screen opens with the location and target.
pub(crate) fn step_started(
    screen: &mut DevScreen,
    (step, detail): (DevStep, String),
    now: Instant,
) -> Vec<String> {
    if step == DevStep::Discover {
        screen.generation_started = Some(now);
    }
    if screen.ready {
        return Vec::new();
    }
    if step == DevStep::Discover {
        for later in [DevStep::Compile, DevStep::Publish] {
            if let Some(row) = step_row(screen, later) {
                row.state = PhaseState::Pending;
                row.detail.clear();
                row.started = None;
                row.elapsed = None;
            }
        }
    }
    if let Some(row) = step_row(screen, step) {
        row.state = PhaseState::Running { frame: 0 };
        row.detail = detail;
        row.started = Some(now);
        row.elapsed = None;
    }
    if step == DevStep::ControlStore && !screen.live {
        return vec![format_plain_line(
            &format!(
                "location {} \u{b7} target {}",
                screen.location, screen.header.target
            ),
            None,
        )];
    }
    Vec::new()
}

/// A startup step ended in `state`; the plain screen reports the control store and the compile.
pub(crate) fn step_finished(
    screen: &mut DevScreen,
    (step, state): (DevStep, PhaseState),
    detail: String,
    now: Instant,
) -> Vec<String> {
    if step == DevStep::Compile {
        screen.compile_detail.clone_from(&detail);
    }
    if screen.ready {
        return Vec::new();
    }
    let elapsed: Option<Duration> = step_row(screen, step).map(|row| {
        let elapsed: Duration = row
            .started
            .map(|started| now.saturating_duration_since(started))
            .unwrap_or_default();
        row.state = state;
        row.detail = detail;
        row.elapsed = Some(elapsed);
        elapsed
    });
    if screen.live {
        return Vec::new();
    }
    match step {
        DevStep::ControlStore => vec![format_plain_line("control store ready", elapsed)],
        DevStep::Compile => {
            let since: Option<Duration> = screen
                .generation_started
                .map(|started| now.saturating_duration_since(started));
            vec![format_plain_line(
                &format!("discovered {}", screen.compile_detail),
                since,
            )]
        }
        _ => Vec::new(),
    }
}

/// An event from the location child.
pub(crate) fn child_event(
    screen: &mut DevScreen,
    event: DevEvent,
    now: Instant,
    clock: &str,
) -> Vec<String> {
    match event {
        DevEvent::Step {
            phase,
            state,
            detail,
        } => {
            let step: DevStep = match phase {
                DevPhase::Discover => DevStep::Discover,
                DevPhase::Compile => DevStep::Compile,
                DevPhase::Publish => DevStep::Publish,
            };
            match state {
                DevStepState::Started => step_started(screen, (step, String::new()), now),
                DevStepState::Finished => step_finished(
                    screen,
                    (step, PhaseState::Ok),
                    detail.unwrap_or_default(),
                    now,
                ),
            }
        }
        DevEvent::Changed { paths, more } => {
            screen.pending_changes += paths.len() + more;
            let mut titles: Vec<String> = paths;
            if more > 0 {
                titles.push(format!("{more} more files"));
            }
            titles
                .into_iter()
                .flat_map(|title| {
                    report_lines(
                        screen,
                        Report {
                            clock,
                            mark: EventMark::Changed,
                            plain: format!("changed {title}"),
                            title,
                            detail: String::new(),
                            elapsed: None,
                            notes: Vec::new(),
                        },
                    )
                })
                .collect()
        }
        DevEvent::Restarted { reason } => report_lines(
            screen,
            Report {
                clock,
                mark: EventMark::Restarted,
                title: "location restarted".to_owned(),
                plain: format!("location restarted: {reason}"),
                detail: reason,
                elapsed: None,
                notes: Vec::new(),
            },
        ),
        DevEvent::Published {
            generation,
            snapshot_id,
            ..
        } => published(screen, (generation, &snapshot_id), now, clock),
        DevEvent::Failed {
            generation,
            message,
            position,
        } => failed(screen, (generation, &message, position.as_ref()), now, clock),
    }
}

fn published(
    screen: &mut DevScreen,
    (generation, snapshot_id): (i64, &str),
    now: Instant,
    clock: &str,
) -> Vec<String> {
    let short: String = snapshot_id.chars().take(SNAPSHOT_ID_CHARS).collect();
    let elapsed: Option<Duration> = screen
        .generation_started
        .take()
        .map(|started| now.saturating_duration_since(started));
    let changes: usize = std::mem::take(&mut screen.pending_changes);
    screen.serving = Some(generation);
    let plain: String = format!("published gen {generation} {short}");
    if !screen.ready {
        let row_elapsed: Option<Duration> = step_row(screen, DevStep::Publish).and_then(|row| {
            row.detail = format!("gen {generation} \u{b7} {short}");
            row.elapsed
        });
        if screen.live {
            return Vec::new();
        }
        return vec![format_plain_line(&plain, row_elapsed)];
    }
    let detail: String = match changes {
        0 => String::new(),
        1 => "1 file changed".to_owned(),
        count => format!("{count} files changed"),
    };
    report_lines(
        screen,
        Report {
            clock,
            mark: EventMark::Succeeded,
            title: format!("gen {generation} published"),
            detail,
            elapsed,
            notes: Vec::new(),
            plain,
        },
    )
}

fn failed(
    screen: &mut DevScreen,
    (generation, message, position): (i64, &str, Option<&SourcePosition>),
    now: Instant,
    clock: &str,
) -> Vec<String> {
    screen.generation_started = None;
    screen.pending_changes = 0;
    if !screen.ready {
        for row in &mut screen.steps {
            if let PhaseState::Running { .. } = row.state {
                row.state = PhaseState::Fail;
                row.elapsed = row
                    .started
                    .map(|started| now.saturating_duration_since(started));
                row.detail = format!("gen {generation} failed");
            }
        }
    }
    let title: String = match screen.serving {
        Some(serving) => format!("gen {generation} failed \u{b7} still serving gen {serving}"),
        None => format!("gen {generation} failed"),
    };
    let mut first_note: Vec<Segment> = Vec::new();
    let mut place: String = String::new();
    if let Some(position) = position {
        place = format!("{}:{}:{}", position.path, position.line, position.column);
        first_note.push(Segment {
            text: place.clone(),
            tone: Tone::Link,
            link: Some(format!(
                "file://{}/{}",
                screen.project_root.trim_end_matches('/'),
                position.path
            )),
        });
        first_note.push(Segment {
            text: "  ".to_owned(),
            tone: Tone::Plain,
            link: None,
        });
        place.push(' ');
    }
    first_note.push(Segment {
        text: message.to_owned(),
        tone: Tone::Plain,
        link: None,
    });
    report_lines(
        screen,
        Report {
            clock,
            mark: EventMark::Failed,
            plain: format!("{title}: {place}{message}"),
            title,
            detail: String::new(),
            elapsed: None,
            notes: vec![
                first_note,
                vec![Segment {
                    text: FAILED_HINT.to_owned(),
                    tone: Tone::Muted,
                    link: None,
                }],
            ],
        },
    )
}

/// The foreground connection settled: close any framed client block and record the outcome.
pub(crate) fn warehouse_settled(
    screen: &mut DevScreen,
    (adapter, state): (String, WarehouseState),
    now: Instant,
    clock: &str,
) -> (String, Vec<String>) {
    let mut raw: String = String::new();
    if screen.client_output {
        if screen.client_partial {
            raw.push('\n');
        }
        if screen.live {
            raw.push_str(&render_frame_rule(None, &screen.facts));
            raw.push_str("\n\n");
        }
        screen.client_output = false;
        screen.client_partial = false;
        screen.drawn = 0;
    }
    screen.adapter.clone_from(&adapter);
    screen.warehouse = Some(state.clone());
    let (row_state, detail, plain): (PhaseState, String, Option<String>) = match &state {
        WarehouseState::Connected { detail } => {
            (PhaseState::Ok, format!("{adapter} \u{b7} {detail}"), None)
        }
        WarehouseState::NotConnected { reason } => (
            PhaseState::Warn,
            format!("not connected \u{b7} {reason}"),
            Some(format!("warehouse not connected: {reason}")),
        ),
        WarehouseState::Failed { message } => (
            PhaseState::Fail,
            message.clone(),
            Some(format!("warehouse connection failed: {message}")),
        ),
    };
    if !screen.ready {
        step_finished(screen, (DevStep::Warehouse, row_state), detail, now);
        let lines: Vec<String> = match (screen.live, plain) {
            (false, Some(plain)) => vec![format_plain_line(&plain, None)],
            _ => Vec::new(),
        };
        return (raw, lines);
    }
    let (mark, title): (EventMark, &str) = match row_state {
        PhaseState::Ok => (EventMark::Succeeded, "warehouse connected"),
        _ => (EventMark::Failed, "warehouse not connected"),
    };
    let lines: Vec<String> = report_lines(
        screen,
        Report {
            clock,
            mark,
            title: title.to_owned(),
            plain: plain.unwrap_or_else(|| format!("warehouse connected: {detail}")),
            detail,
            elapsed: None,
            notes: Vec::new(),
        },
    );
    (raw, lines)
}

/// The client's own output, written as it came; the first chunk freezes the checklist above a rule.
pub(crate) fn client_text(screen: &mut DevScreen, text: &str, now: Instant) -> String {
    let mut output: String = String::new();
    if !screen.client_output {
        screen.client_output = true;
        if screen.live {
            let rows: Vec<String> = live_rows(screen, now);
            output.push_str(&redraw_rows(screen.drawn, &rows));
            screen.drawn = 0;
        }
    }
    if !text.is_empty() {
        screen.client_partial = !text.ends_with('\n');
    }
    output.push_str(text);
    output
}

/// A run started or finished.
pub(crate) fn run_reported(
    screen: &DevScreen,
    (name, detail): (String, String),
    finished: Option<(bool, Duration)>,
    clock: &str,
) -> Vec<String> {
    let (mark, plain, elapsed): (EventMark, String, Option<Duration>) = match finished {
        None => (
            EventMark::Started,
            format!("run {name} started: {detail}"),
            None,
        ),
        Some((true, elapsed)) => (
            EventMark::Succeeded,
            format!("run {name} succeeded: {detail}"),
            Some(elapsed),
        ),
        Some((false, elapsed)) => (
            EventMark::Failed,
            format!("run {name} failed: {detail}"),
            Some(elapsed),
        ),
    };
    report_lines(
        screen,
        Report {
            clock,
            mark,
            title: format!("run {name}"),
            detail,
            elapsed,
            notes: Vec::new(),
            plain,
        },
    )
}

/// Keep `line` in the ring, and show it when the log is open.
pub(crate) fn logged(screen: &mut DevScreen, line: String) -> Vec<String> {
    if screen.log.len() == LOG_RING_LINES {
        screen.log.pop_front();
    }
    let shown: Vec<String> = if screen.live && screen.show_log {
        vec![render_log_line(&line, &screen.facts)]
    } else {
        Vec::new()
    };
    screen.log.push_back(line);
    shown
}

/// What a key asks for; `l` toggles the log and opens it with the lines kept so far.
pub(crate) fn key_pressed(screen: &mut DevScreen, key: char) -> (Vec<String>, ScreenAction) {
    if !screen.live {
        return (Vec::new(), ScreenAction::None);
    }
    let action: ScreenAction = match key {
        'o' => ScreenAction::OpenUi,
        'r' => ScreenAction::Recompile,
        's' => ScreenAction::RetryWarehouse,
        'q' => ScreenAction::Quit,
        'l' => {
            screen.show_log = !screen.show_log;
            if screen.show_log {
                let lines: Vec<String> = screen
                    .log
                    .iter()
                    .map(|line| render_log_line(line, &screen.facts))
                    .collect();
                return (lines, ScreenAction::None);
            }
            ScreenAction::None
        }
        _ => ScreenAction::None,
    };
    (Vec::new(), action)
}
