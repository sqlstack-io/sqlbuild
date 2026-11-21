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

use crate::dev_screen::_helpers::lines::{Report, report_lines};
use crate::dev_screen::_helpers::rows::{elapsed_since, live_rows};
use crate::dev_screen::constants::{FAILED_HINT, LOG_RING_LINES, SNAPSHOT_ID_CHARS};
use crate::dev_screen::models::{DevScreen, DevStep, ScreenAction, StepRow};

/// The screen after a message, and the lines it prints for good.
pub(crate) type Printed = (DevScreen, Vec<String>);

/// A startup step began; discovery starts a new generation, so its later steps go back to pending.
pub(crate) fn step_started(
    mut screen: DevScreen,
    (step, detail): (DevStep, String),
    now: Instant,
) -> Printed {
    if step == DevStep::Discover {
        screen.generation_started = Some(now);
    }
    if screen.ready {
        return (screen, Vec::new());
    }
    let reset: &[DevStep] = if step == DevStep::Discover {
        &[DevStep::Compile, DevStep::Publish]
    } else {
        &[]
    };
    for row in &mut screen.steps {
        if reset.contains(&row.step) {
            *row = pending(row.step);
        }
        if row.step == step {
            *row = StepRow {
                state: PhaseState::Running { frame: 0 },
                detail: detail.clone(),
                started: Some(now),
                ..pending(step)
            };
        }
    }
    (screen, Vec::new())
}

/// A checklist row that has not started.
pub(crate) fn pending(step: DevStep) -> StepRow {
    StepRow {
        step,
        state: PhaseState::Pending,
        detail: String::new(),
        started: None,
        elapsed: None,
    }
}

/// The location is known; the plain screen names it and the target.
pub(crate) fn located(mut screen: DevScreen, location: String) -> Printed {
    screen.location = location;
    if screen.live {
        return (screen, Vec::new());
    }
    let line: String = format_plain_line(
        &format!(
            "location {} \u{b7} target {}",
            screen.location, screen.header.target
        ),
        None,
    );
    (screen, vec![line])
}

/// A startup step ended in `state`; the plain screen reports the control store and the compile.
pub(crate) fn step_finished(
    mut screen: DevScreen,
    (step, state): (DevStep, PhaseState),
    detail: String,
    now: Instant,
) -> Printed {
    match step {
        DevStep::Compile => screen.compile_detail.clone_from(&detail),
        DevStep::Api => screen.ui_url = format!("http://{detail}"),
        _ => {}
    }
    if screen.ready {
        return (screen, Vec::new());
    }
    let mut elapsed: Option<Duration> = None;
    for row in &mut screen.steps {
        if row.step == step {
            row.elapsed = Some(elapsed_since(row.started, now).unwrap_or_default());
            row.state = state;
            row.detail.clone_from(&detail);
            elapsed = row.elapsed;
        }
    }
    if screen.live {
        return (screen, Vec::new());
    }
    let lines: Vec<String> = match step {
        DevStep::ControlStore => vec![format_plain_line("control store ready", elapsed)],
        DevStep::Compile => vec![format_plain_line(
            &format!("discovered {}", screen.compile_detail),
            elapsed_since(screen.generation_started, now),
        )],
        _ => Vec::new(),
    };
    (screen, lines)
}

/// An event from the location child.
pub(crate) fn child_event(
    mut screen: DevScreen,
    event: DevEvent,
    now: Instant,
    clock: &str,
) -> Printed {
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
            let lines: Vec<String> = titles
                .into_iter()
                .flat_map(|title| changed_line(&screen, clock, title))
                .collect();
            (screen, lines)
        }
        DevEvent::Restarted { reason } => {
            let lines: Vec<String> = report_lines(
                &screen,
                Report {
                    clock,
                    mark: EventMark::Restarted,
                    title: "location restarted".to_owned(),
                    plain: format!("location restarted: {reason}"),
                    detail: reason,
                    elapsed: None,
                    notes: Vec::new(),
                },
            );
            (screen, lines)
        }
        DevEvent::Published {
            generation,
            snapshot_id,
            ..
        } => published(screen, (generation, &snapshot_id), now, clock),
        DevEvent::Failed {
            generation,
            message,
            position,
        } => failed(
            screen,
            (generation, &message, position.as_ref()),
            now,
            clock,
        ),
        DevEvent::Discovered { summary } => {
            let changed: bool = screen.discovered.as_ref() != Some(&summary);
            let lines: Vec<String> = if screen.ready && changed {
                report_lines(
                    &screen,
                    Report {
                        clock,
                        mark: EventMark::Succeeded,
                        title: "discovered".to_owned(),
                        plain: format!("discovered {summary}"),
                        detail: summary.clone(),
                        elapsed: None,
                        notes: Vec::new(),
                    },
                )
            } else {
                Vec::new()
            };
            (
                DevScreen {
                    discovered: Some(summary),
                    ..screen
                },
                lines,
            )
        }
        DevEvent::Warning { title, detail } => {
            let lines: Vec<String> = report_lines(
                &screen,
                Report {
                    clock,
                    mark: EventMark::Warning,
                    plain: format!("{title}: {detail}"),
                    title,
                    detail: String::new(),
                    elapsed: None,
                    notes: vec![vec![Segment {
                        text: detail,
                        tone: Tone::Muted,
                        link: None,
                    }]],
                },
            );
            (screen, lines)
        }
    }
}

fn changed_line(screen: &DevScreen, clock: &str, title: String) -> Vec<String> {
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
}

fn published(
    mut screen: DevScreen,
    (generation, snapshot_id): (i64, &str),
    now: Instant,
    clock: &str,
) -> Printed {
    let short: String = snapshot_id.chars().take(SNAPSHOT_ID_CHARS).collect();
    let elapsed: Option<Duration> = elapsed_since(screen.generation_started.take(), now);
    let changes: usize = std::mem::take(&mut screen.pending_changes);
    screen.serving = Some(generation);
    let plain: String = format!("published gen {generation} {short}");
    if !screen.ready {
        let mut row_elapsed: Option<Duration> = None;
        for row in &mut screen.steps {
            if row.step == DevStep::Publish {
                row.detail = format!("gen {generation} \u{b7} {short}");
                row_elapsed = row.elapsed;
            }
        }
        if screen.live {
            return (screen, Vec::new());
        }
        return (screen, vec![format_plain_line(&plain, row_elapsed)]);
    }
    let detail: String = match changes {
        0 => String::new(),
        1 => "1 file changed".to_owned(),
        count => format!("{count} files changed"),
    };
    let lines: Vec<String> = report_lines(
        &screen,
        Report {
            clock,
            mark: EventMark::Succeeded,
            title: format!("gen {generation} published"),
            detail,
            elapsed,
            notes: Vec::new(),
            plain,
        },
    );
    (screen, lines)
}

fn failed(
    mut screen: DevScreen,
    (generation, message, position): (i64, &str, Option<&SourcePosition>),
    now: Instant,
    clock: &str,
) -> Printed {
    screen.generation_started = None;
    screen.pending_changes = 0;
    for row in &mut screen.steps {
        if !screen.ready && matches!(row.state, PhaseState::Running { .. }) {
            row.state = PhaseState::Fail;
            row.elapsed = elapsed_since(row.started, now);
            row.detail = format!("gen {generation} failed");
        }
    }
    let title: String = match screen.serving {
        Some(serving) => format!("gen {generation} failed \u{b7} still serving gen {serving}"),
        None => format!("gen {generation} failed"),
    };
    let (place, mut first_note): (String, Vec<Segment>) = match position {
        Some(position) => position_note(&screen.project_root, position),
        None => (String::new(), Vec::new()),
    };
    first_note.push(Segment {
        text: message.to_owned(),
        tone: Tone::Plain,
        link: None,
    });
    let lines: Vec<String> = report_lines(
        &screen,
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
    );
    (screen, lines)
}

/// `path:line:col ` for the plain line, and the clickable position the live note starts with.
fn position_note(project_root: &str, position: &SourcePosition) -> (String, Vec<Segment>) {
    let place: String = format!("{}:{}:{}", position.path, position.line, position.column);
    let note: Vec<Segment> = vec![
        Segment {
            text: place.clone(),
            tone: Tone::Link,
            link: Some(format!(
                "file://{}/{}",
                project_root.trim_end_matches('/'),
                position.path
            )),
        },
        Segment {
            text: "  ".to_owned(),
            tone: Tone::Plain,
            link: None,
        },
    ];
    (format!("{place} "), note)
}

/// The foreground connection settled: close any framed client block and record the outcome.
pub(crate) fn warehouse_settled(
    mut screen: DevScreen,
    (adapter, state): (String, WarehouseState),
    now: Instant,
    clock: &str,
) -> (DevScreen, String, Vec<String>) {
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
        let (screen, _) = step_finished(screen, (DevStep::Warehouse, row_state), detail, now);
        let lines: Vec<String> = match (screen.live, plain) {
            (false, Some(plain)) => vec![format_plain_line(&plain, None)],
            _ => Vec::new(),
        };
        return (screen, raw, lines);
    }
    let (mark, title): (EventMark, &str) = match row_state {
        PhaseState::Ok => (EventMark::Succeeded, "warehouse connected"),
        _ => (EventMark::Failed, "warehouse not connected"),
    };
    let lines: Vec<String> = report_lines(
        &screen,
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
    (screen, raw, lines)
}

/// The client's own output, written as it came; the first chunk freezes the checklist above a rule.
pub(crate) fn client_text(mut screen: DevScreen, text: &str, now: Instant) -> (DevScreen, String) {
    let mut output: String = String::new();
    if !screen.client_output {
        screen.client_output = true;
        if screen.live {
            let rows: Vec<String> = live_rows(&screen, now);
            output.push_str(&redraw_rows(screen.drawn, &rows));
            screen.drawn = 0;
        }
    }
    if !text.is_empty() {
        screen.client_partial = !text.ends_with('\n');
    }
    output.push_str(text);
    (screen, output)
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
pub(crate) fn logged(mut screen: DevScreen, line: String) -> Printed {
    if screen.log.len() == LOG_RING_LINES {
        screen.log.pop_front();
    }
    let shown: Vec<String> = if screen.live && screen.show_log {
        vec![render_log_line(&line, &screen.facts)]
    } else {
        Vec::new()
    };
    screen.log.push_back(line);
    (screen, shown)
}

/// What a key asks for; `l` toggles the log and opens it with the lines kept so far.
pub(crate) fn key_pressed(
    mut screen: DevScreen,
    key: char,
) -> (DevScreen, Vec<String>, ScreenAction) {
    if !screen.live {
        return (screen, Vec::new(), ScreenAction::None);
    }
    let action: ScreenAction = match key {
        'o' => ScreenAction::OpenUi,
        'r' => ScreenAction::Recompile,
        's' => ScreenAction::RetryWarehouse,
        'q' => ScreenAction::Quit,
        _ => ScreenAction::None,
    };
    if key != 'l' {
        return (screen, Vec::new(), action);
    }
    screen.show_log = !screen.show_log;
    let shown: Vec<String> = if screen.show_log {
        screen
            .log
            .iter()
            .map(|line| render_log_line(line, &screen.facts))
            .collect()
    } else {
        Vec::new()
    };
    (screen, shown, action)
}
