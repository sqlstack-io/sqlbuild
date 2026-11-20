//! How each message changes the `sqb dev` screen, and the lines it prints for good.

use std::time::{Duration, Instant};

use sqlbuild_core::screen_text::main::render_event_line::render_event_line;
use sqlbuild_core::screen_text::main::render_log_line::render_log_line;
use sqlbuild_core::screen_text::main::render_problem::render_problem;
use sqlbuild_core::screen_text::models::{
    EventLine, EventMark, PhaseState, Problem, ProblemLevel, Segment, StepLine, Tone,
    WarehouseState,
};
use sqlbuild_orchestration::dev_events::models::{
    DevEvent, DevPhase, DevStepState, SourcePosition,
};

use crate::dev_screen::_helpers::labels::elapsed_since;
use crate::dev_screen::_helpers::lines::lasting_step;
use crate::dev_screen::_helpers::rows::step_line;
use crate::dev_screen::constants::{LOG_RING_LINES, SIGN_IN_DETAIL, SNAPSHOT_ID_CHARS};
use crate::dev_screen::models::{DevScreen, DevStep, Prompt, ScreenAction, StepRow};

/// The screen after a message, and the lines it prints for good.
pub(crate) type Printed = (DevScreen, Vec<String>);

/// The most client text a sign-in prompt keeps; older text drops off the front.
const PROMPT_TEXT_LIMIT: usize = 4_000;

/// A step began; discovery starts a new generation, so its later steps go back to pending.
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

/// A step that has not started.
pub(crate) fn pending(step: DevStep) -> StepRow {
    StepRow {
        step,
        state: PhaseState::Pending,
        detail: String::new(),
        started: None,
        elapsed: None,
    }
}

/// A step ended in `state`: its line is printed for good, except publishing, which waits for the snapshot ID.
pub(crate) fn step_finished(
    mut screen: DevScreen,
    (step, state): (DevStep, PhaseState),
    detail: String,
    now: Instant,
) -> Printed {
    let detail: String = match step {
        DevStep::Compile => {
            screen.compile_detail.clone_from(&detail);
            detail
        }
        DevStep::Api => {
            screen.ui_url = format!("http://{detail}");
            screen.ui_url.clone()
        }
        _ => detail,
    };
    if screen.ready {
        return (screen, Vec::new());
    }
    let Some(index) = screen.steps.iter().position(|row| row.step == step) else {
        return (screen, Vec::new());
    };
    let row: &mut StepRow = &mut screen.steps[index];
    row.elapsed = Some(elapsed_since(row.started, now).unwrap_or_default());
    row.state = state;
    row.detail = detail;
    if step == DevStep::Publish && screen.serving.is_none() {
        return (screen, Vec::new());
    }
    let line: StepLine = step_line(&screen.steps[index], 0, now);
    let lines: Vec<String> = lasting_step(&screen, &line);
    (screen, lines)
}

/// A step of `sqb dev` itself failed: its line, then the problem under it.
pub(crate) fn step_failed(
    mut screen: DevScreen,
    (step, detail, problem): (DevStep, String, Problem),
    now: Instant,
) -> Printed {
    let mut row: StepRow = screen
        .steps
        .iter()
        .find(|row| row.step == step)
        .cloned()
        .unwrap_or_else(|| StepRow {
            started: Some(now),
            ..pending(step)
        });
    row.state = PhaseState::Fail;
    row.elapsed = Some(elapsed_since(row.started, now).unwrap_or_default());
    row.detail = detail;
    let mut lines: Vec<String> = lasting_step(&screen, &step_line(&row, 0, now));
    lines.extend(render_problem(&problem, &screen.facts));
    for kept in &mut screen.steps {
        if kept.step == step {
            kept.clone_from(&row);
        }
    }
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
                .flat_map(|title| {
                    event_lines(
                        &screen,
                        event_line(
                            clock,
                            EventMark::Changed,
                            format!("{title} changed"),
                            String::new(),
                        ),
                    )
                })
                .collect();
            (screen, lines)
        }
        DevEvent::Restarted { reason } => {
            let lines: Vec<String> = event_lines(
                &screen,
                event_line(
                    clock,
                    EventMark::Restarted,
                    "location restarted".to_owned(),
                    reason,
                ),
            );
            (screen, lines)
        }
        DevEvent::Published { snapshot_id, .. } => published(screen, &snapshot_id, now, clock),
        DevEvent::Failed {
            message,
            position,
            code,
            help,
            ..
        } => {
            let problem: Problem = failure_problem(
                &screen.project_root,
                (&message, position.as_ref()),
                (code, help),
            );
            failed(screen, problem, now, clock)
        }
        DevEvent::Warning { title, detail } => {
            let lines: Vec<String> = render_problem(
                &Problem {
                    level: ProblemLevel::Warning,
                    title,
                    code: String::new(),
                    at: Vec::new(),
                    why: detail,
                    fix: String::new(),
                },
                &screen.facts,
            );
            (screen, lines)
        }
    }
}

/// An event line without elapsed time or notes.
fn event_line(clock: &str, mark: EventMark, title: String, detail: String) -> EventLine {
    EventLine {
        clock: clock.to_owned(),
        mark,
        title,
        detail,
        elapsed: None,
        notes: Vec::new(),
    }
}

/// The event's rows, wrapped in full in both live and plain mode.
pub(crate) fn event_lines(screen: &DevScreen, line: EventLine) -> Vec<String> {
    render_event_line(&line, &screen.facts)
}

fn published(mut screen: DevScreen, snapshot_id: &str, now: Instant, clock: &str) -> Printed {
    let short: String = snapshot_id.chars().take(SNAPSHOT_ID_CHARS).collect();
    let elapsed: Option<Duration> = elapsed_since(screen.generation_started.take(), now);
    let changes: usize = std::mem::take(&mut screen.pending_changes);
    screen.serving = Some(short.clone());
    if !screen.ready {
        let Some(index) = screen
            .steps
            .iter()
            .position(|row| row.step == DevStep::Publish)
        else {
            return (screen, Vec::new());
        };
        let row: &mut StepRow = &mut screen.steps[index];
        row.state = PhaseState::Ok;
        row.elapsed = row
            .elapsed
            .or_else(|| Some(elapsed_since(row.started, now).unwrap_or_default()));
        row.detail = format!("snapshot {short}");
        let line: StepLine = step_line(&screen.steps[index], 0, now);
        let lines: Vec<String> = lasting_step(&screen, &line);
        return (screen, lines);
    }
    let detail: String = match changes {
        0 => String::new(),
        1 => "1 file changed".to_owned(),
        count => format!("{count} files changed"),
    };
    let lines: Vec<String> = event_lines(
        &screen,
        EventLine {
            elapsed,
            ..event_line(
                clock,
                EventMark::Succeeded,
                format!("published snapshot {short}"),
                detail,
            )
        },
    );
    (screen, lines)
}

/// The problem block for a failed generation: the file it points at, its code, the message and the fix.
pub(crate) fn failure_problem(
    project_root: &str,
    (message, position): (&str, Option<&SourcePosition>),
    (code, help): (Option<String>, Option<String>),
) -> Problem {
    let code: String = code.unwrap_or_default();
    let prefix: String = format!("[{code}] ");
    let mut why: &str = message.strip_prefix(prefix.as_str()).unwrap_or(message);
    if let Some(position) = position {
        for place in [
            format!("{}:{}:{}:", position.path, position.line, position.column),
            format!("{}:{}:", position.path, position.line),
        ] {
            why = why.strip_prefix(place.as_str()).unwrap_or(why);
        }
    }
    let (title, at): (String, Vec<Segment>) = match position {
        Some(position) => (
            position.path.clone(),
            vec![Segment {
                text: format!("{}:{}:{}", position.path, position.line, position.column),
                tone: Tone::Link,
                link: Some(format!(
                    "file://{}/{}",
                    project_root.trim_end_matches('/'),
                    position.path
                )),
            }],
        ),
        None => ("project".to_owned(), Vec::new()),
    };
    Problem {
        level: ProblemLevel::Error,
        title,
        code,
        at,
        why: why.trim().to_owned(),
        fix: help.unwrap_or_else(|| {
            "fix it and save; sqb dev compiles again (r recompiles now)".to_owned()
        }),
    }
}

/// A generation failed: before ready, the running steps fail and wait for a fix; after, one event line. The problem follows in full.
fn failed(mut screen: DevScreen, problem: Problem, now: Instant, clock: &str) -> Printed {
    screen.generation_started = None;
    screen.pending_changes = 0;
    let mut lines: Vec<String> = Vec::new();
    if screen.ready {
        let title: String = match &screen.serving {
            Some(serving) => format!("compile failed \u{b7} still serving snapshot {serving}"),
            None => "compile failed".to_owned(),
        };
        lines.extend(event_lines(
            &screen,
            event_line(clock, EventMark::Failed, title, String::new()),
        ));
    } else {
        for index in 0..screen.steps.len() {
            if !matches!(screen.steps[index].state, PhaseState::Running { .. })
                || screen.steps[index].step == DevStep::Warehouse
            {
                continue;
            }
            let mut row: StepRow = screen.steps[index].clone();
            row.state = PhaseState::Fail;
            row.elapsed = elapsed_since(row.started, now);
            row.detail = "1 error".to_owned();
            lines.extend(lasting_step(&screen, &step_line(&row, 0, now)));
        }
        for row in &mut screen.steps {
            if matches!(
                row.step,
                DevStep::Discover | DevStep::Compile | DevStep::Publish
            ) {
                *row = pending(row.step);
            }
            if row.step == DevStep::Discover {
                row.detail = "waiting for a fix (r recompiles)".to_owned();
            }
        }
    }
    lines.extend(render_problem(&problem, &screen.facts));
    (screen, lines)
}

/// The foreground connection settled: the prompt goes, and its step line or event line is printed.
pub(crate) fn warehouse_settled(
    mut screen: DevScreen,
    (adapter, state): (String, WarehouseState),
    now: Instant,
    clock: &str,
) -> Printed {
    screen.prompt = None;
    if !adapter.is_empty() {
        screen.adapter.clone_from(&adapter);
    }
    screen.warehouse = Some(state.clone());
    let adapter: String = screen.adapter.clone();
    let (row_state, detail): (PhaseState, String) = match &state {
        WarehouseState::Connected { facts } => (
            PhaseState::Ok,
            [adapter.clone()]
                .into_iter()
                .chain(
                    facts
                        .iter()
                        .filter_map(|fact| fact.strip_prefix("auth ").map(str::to_owned)),
                )
                .filter(|part| !part.is_empty())
                .collect::<Vec<String>>()
                .join(" \u{b7} "),
        ),
        WarehouseState::NotConnected { reason } => {
            (PhaseState::Warn, format!("not connected \u{b7} {reason}"))
        }
        WarehouseState::Failed { .. } => (
            PhaseState::Fail,
            [adapter.as_str(), "failed"]
                .into_iter()
                .filter(|part| !part.is_empty())
                .collect::<Vec<&str>>()
                .join(" \u{b7} "),
        ),
    };
    let mut lines: Vec<String> = Vec::new();
    if screen.ready {
        let (mark, title): (EventMark, &str) = match row_state {
            PhaseState::Ok => (EventMark::Succeeded, "warehouse connected"),
            _ => (EventMark::Failed, "warehouse not connected"),
        };
        lines.extend(event_lines(
            &screen,
            event_line(clock, mark, title.to_owned(), detail),
        ));
    } else {
        let printed: Printed = step_finished(screen, (DevStep::Warehouse, row_state), detail, now);
        screen = printed.0;
        lines.extend(printed.1);
    }
    if let WarehouseState::Failed { message } = &state {
        lines.extend(render_problem(
            &Problem {
                level: ProblemLevel::Error,
                title: "warehouse connection".to_owned(),
                code: adapter,
                at: vec![Segment {
                    text: format!("target {}", screen.header.target),
                    tone: Tone::Plain,
                    link: None,
                }],
                why: message.clone(),
                fix: "sqb debug shows the whole connection check; fix the target's connection \
                      settings, then start sqb dev again"
                    .to_owned(),
            },
            &screen.facts,
        ));
    }
    (screen, lines)
}

/// The client's own text: kept in the prompt block above the live steps until the connection settles.
pub(crate) fn client_text(mut screen: DevScreen, text: &str, now: Instant) -> DevScreen {
    let prompt: &mut Prompt = screen.prompt.get_or_insert(Prompt {
        text: String::new(),
        since: now,
    });
    prompt.text = typed(std::mem::take(&mut prompt.text), text);
    if prompt.text.len() > PROMPT_TEXT_LIMIT {
        let mut start: usize = prompt.text.len() - PROMPT_TEXT_LIMIT;
        while !prompt.text.is_char_boundary(start) {
            start += 1;
        }
        prompt.text.drain(..start);
    }
    for row in &mut screen.steps {
        if row.step == DevStep::Warehouse && matches!(row.state, PhaseState::Running { .. }) {
            row.detail = SIGN_IN_DETAIL.to_owned();
        }
    }
    screen
}

/// `buffer` with `text` appended as a terminal would show it: backspaces erase, escape sequences and carriage returns go.
fn typed(mut buffer: String, text: &str) -> String {
    let mut characters = text.chars().peekable();
    while let Some(character) = characters.next() {
        match character {
            '\u{8}' | '\u{7f}' => {
                if !buffer.ends_with('\n') {
                    buffer.pop();
                }
            }
            '\u{1b}' => {
                if characters.peek() == Some(&'[') {
                    characters.next();
                    for next in characters.by_ref() {
                        if next.is_ascii_alphabetic() {
                            break;
                        }
                    }
                }
            }
            '\r' => {}
            character => buffer.push(character),
        }
    }
    buffer
}

/// A run started or finished.
pub(crate) fn run_reported(
    screen: &DevScreen,
    (name, detail): (String, String),
    finished: Option<(bool, Duration)>,
    clock: &str,
) -> Vec<String> {
    let (mark, elapsed): (EventMark, Option<Duration>) = match finished {
        None => (EventMark::Started, None),
        Some((true, elapsed)) => (EventMark::Succeeded, Some(elapsed)),
        Some((false, elapsed)) => (EventMark::Failed, Some(elapsed)),
    };
    event_lines(
        screen,
        EventLine {
            elapsed,
            ..event_line(clock, mark, format!("run {name}"), detail)
        },
    )
}

/// Keep `line` in the ring, and show it when the log is open.
pub(crate) fn logged(mut screen: DevScreen, line: String) -> Printed {
    if screen.log.len() == LOG_RING_LINES {
        screen.log.pop_front();
    }
    let shown: Vec<String> = if screen.show_log {
        render_log_line(&line, &screen.facts)
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
    let action: ScreenAction = match key {
        'o' => ScreenAction::OpenUi,
        'r' => ScreenAction::Recompile,
        's' => ScreenAction::RetryWarehouse,
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
            .flat_map(|line| render_log_line(line, &screen.facts))
            .collect()
    } else {
        Vec::new()
    };
    (screen, shown, action)
}

/// A service stopped: `· Stopped UI server  0.1s`.
pub(crate) fn service_stopped(screen: &DevScreen, label: String, elapsed: Duration) -> Vec<String> {
    lasting_step(
        screen,
        &StepLine {
            state: PhaseState::Pending,
            label,
            elapsed: Some(elapsed),
            detail: String::new(),
        },
    )
}
