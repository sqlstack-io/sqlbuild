//! Screens at fixed sizes, the messages of a session, and a terminal that replays the redraws into a viewport.

use std::time::{Duration, Instant};

use sqlbuild_core::cli_text::models::CliStyle;
use sqlbuild_core::screen_text::models::{
    DevHeader, Problem, ProblemLevel, ResultLine, ScreenFacts, WarehouseState,
};
use sqlbuild_core::text::main::cell_len::cell_len;
use sqlbuild_orchestration::dev_events::models::{
    DevEvent, DevPhase, DevStepState, SourcePosition,
};

use crate::dev_screen::_helpers::apply_screen_message::apply_screen_message;
use crate::dev_screen::_helpers::drawing::closing_text;
use crate::dev_screen::_helpers::new_dev_screen::new_dev_screen;
use crate::dev_screen::models::{
    DevScreen, DevStep, ScreenAction, ScreenEnd, ScreenMessage, ScreenSetup,
};
use crate::dev_screen::tests::test_types::{Terminal, Transcript};

/// An uncoloured screen `size` big (columns, rows); `None` makes it plain.
pub(super) fn screen(size: Option<(usize, usize)>) -> DevScreen {
    let (columns, rows): (usize, usize) = size.unwrap_or((80, 24));
    new_dev_screen(ScreenSetup {
        live: size.is_some(),
        facts: ScreenFacts {
            columns,
            rows,
            style: CliStyle { use_color: false },
            links: false,
        },
        header: DevHeader {
            version: "0.256.0".to_owned(),
            project: "shop".to_owned(),
            target: "dev".to_owned(),
        },
        adapter: "snowflake".to_owned(),
        project_root: "/home/analyst/projects/shop".to_owned(),
        env_file: None,
    })
}

pub(super) fn step(phase: DevPhase, state: DevStepState, detail: Option<&str>) -> ScreenMessage {
    ScreenMessage::Child(DevEvent::Step {
        phase,
        state,
        detail: detail.map(str::to_owned),
    })
}

fn started(step: DevStep) -> ScreenMessage {
    ScreenMessage::Started {
        step,
        detail: String::new(),
    }
}

fn finished(step: DevStep, detail: &str) -> ScreenMessage {
    ScreenMessage::Finished {
        step,
        detail: detail.to_owned(),
    }
}

/// The front's own steps, then discovery running.
pub(super) fn discovering() -> Vec<ScreenMessage> {
    vec![
        started(DevStep::ControlStore),
        finished(DevStep::ControlStore, "sqlite"),
        started(DevStep::Api),
        finished(DevStep::Api, "127.0.0.1:8640"),
        started(DevStep::Scheduler),
        finished(DevStep::Scheduler, "triggers start paused"),
        step(DevPhase::Discover, DevStepState::Started, None),
        ScreenMessage::Tick,
    ]
}

/// Start-up up to the first snapshot, then the warehouse connection running.
pub(super) fn connecting() -> Vec<ScreenMessage> {
    let mut messages: Vec<ScreenMessage> = discovering();
    messages.extend([
        step(
            DevPhase::Discover,
            DevStepState::Finished,
            Some("118 models \u{b7} 24 seeds \u{b7} 2 Python nodes"),
        ),
        step(DevPhase::Compile, DevStepState::Started, None),
        step(
            DevPhase::Compile,
            DevStepState::Finished,
            Some("0 errors \u{b7} 1 warning"),
        ),
        step(DevPhase::Publish, DevStepState::Started, None),
        step(DevPhase::Publish, DevStepState::Finished, None),
        ScreenMessage::Child(DevEvent::Published {
            generation: 1,
            snapshot_id: "b429729b5c1e".to_owned(),
            record: serde_json::Value::Null,
        }),
        ScreenMessage::Triggers("41 paused".to_owned()),
        ScreenMessage::Started {
            step: DevStep::Warehouse,
            detail: "connecting".to_owned(),
        },
        ScreenMessage::Tick,
    ]);
    messages
}

/// Snowflake's OAuth sign-in prompt while the connection waits.
pub(super) fn signing_in() -> Vec<ScreenMessage> {
    let mut messages: Vec<ScreenMessage> = connecting();
    messages.extend([
        ScreenMessage::ClientOutput(
            "Go to https://xy12345.snowflakecomputing.com/oauth/authorize?client_id=SQLBUILD&state=8f3a21c0&redirect_uri=http%3A%2F%2Flocalhost%3A8001 to sign in\n"
                .to_owned(),
        ),
        ScreenMessage::ClientOutput("Enter the URL the OAuth flow redirected you to: ".to_owned()),
        ScreenMessage::ClientOutput("http://localhost:8001/?code=ab".to_owned()),
        ScreenMessage::Tick,
    ]);
    messages
}

/// Start-up through a connected warehouse: the ready box.
pub(super) fn ready() -> Vec<ScreenMessage> {
    let mut messages: Vec<ScreenMessage> = signing_in();
    messages.push(ScreenMessage::Warehouse {
        adapter: "snowflake".to_owned(),
        state: WarehouseState::Connected {
            facts: [
                "account xy12345",
                "auth browser sign-in (OAuth)",
                "role TRANSFORMER",
                "warehouse TRANSFORM_XS",
            ]
            .map(str::to_owned)
            .to_vec(),
        },
    });
    messages
}

/// Discovery fails with a long error that names its file, code and fix.
pub(super) fn failed_compile() -> Vec<ScreenMessage> {
    let mut messages: Vec<ScreenMessage> = discovering();
    messages.push(ScreenMessage::Child(DevEvent::Failed {
        generation: 1,
        message:
            "[D001] models/marts/fct_orders.sql:42:7: column net_amount_usd does not exist in \
                  stg_payments; the column was renamed to amount_usd_net when the payments source \
                  moved to the v2 export, and fct_orders still selects the old name"
                .to_owned(),
        position: Some(SourcePosition {
            path: "models/marts/fct_orders.sql".to_owned(),
            line: 42,
            column: 7,
        }),
        code: Some("D001".to_owned()),
        help: Some("select amount_usd_net instead, or alias it as net_amount_usd".to_owned()),
    }));
    messages.push(ScreenMessage::Tick);
    messages
}

/// The configuration names a key that was removed, before anything else starts.
pub(super) fn removed_key() -> Vec<ScreenMessage> {
    vec![ScreenMessage::Failed {
        step: DevStep::Config,
        detail: "1 error".to_owned(),
        problem: Problem {
            level: ProblemLevel::Error,
            title: "sqlbuild_project.toml".to_owned(),
            code: "D001".to_owned(),
            at: Vec::new(),
            why: "[dbt] option(s) were removed: defer_clone_from; use dbt-native --state/--defer"
                .to_owned(),
            fix: "remove defer_clone_from from [dbt]".to_owned(),
        },
    }]
}

/// Ctrl+C once ready: the services stop one by one.
pub(super) fn interrupted() -> Vec<ScreenMessage> {
    let mut messages: Vec<ScreenMessage> = ready();
    messages.extend([
        ScreenMessage::Stopping,
        ScreenMessage::Tick,
        ScreenMessage::Stopped {
            label: "Stopped code location".to_owned(),
            elapsed: Duration::from_millis(300),
        },
        ScreenMessage::Stopped {
            label: "Stopped scheduler".to_owned(),
            elapsed: Duration::from_millis(400),
        },
        ScreenMessage::Stopped {
            label: "Stopped UI server".to_owned(),
            elapsed: Duration::from_millis(500),
        },
    ]);
    messages
}

/// The result line Ctrl+C leaves.
pub(super) fn interrupted_end() -> ScreenEnd {
    ScreenEnd {
        result: ResultLine {
            failed: false,
            title: "sqb dev stopped (interrupted)".to_owned(),
            detail: "ran 42m 10s".to_owned(),
            elapsed: Some(Duration::from_millis(1_100)),
            next: Vec::new(),
        },
        show_log: false,
    }
}

/// The result line a configuration error leaves.
pub(super) fn not_started_end() -> ScreenEnd {
    ScreenEnd {
        result: ResultLine {
            failed: true,
            title: "sqb dev did not start".to_owned(),
            detail: "configuration error in sqlbuild_project.toml".to_owned(),
            elapsed: Some(Duration::from_millis(100)),
            next: vec![(
                "sqb debug".to_owned(),
                "check the configuration on its own".to_owned(),
            )],
        },
        show_log: false,
    }
}

/// Nothing: the session is still running.
pub(super) fn still_running(_screen: DevScreen) -> String {
    String::new()
}

/// The closing text Ctrl+C leaves.
pub(super) fn close_interrupted(screen: DevScreen) -> String {
    closing_text(screen, Some(&interrupted_end()))
}

/// The closing text a configuration error leaves.
pub(super) fn close_not_started(screen: DevScreen) -> String {
    closing_text(screen, Some(&not_started_end()))
}

/// Apply `messages` 100 ms apart at `size`, then write what `close` leaves; what the terminal shows and keeps.
pub(super) fn play(
    size: Option<(usize, usize)>,
    messages: Vec<ScreenMessage>,
    close: fn(DevScreen) -> String,
) -> Transcript {
    let start: Instant = Instant::now();
    let mut screen: DevScreen = screen(size);
    let mut terminal: Terminal = Terminal::new(size.map_or(usize::MAX, |(_, rows)| rows));
    let mut actions: Vec<ScreenAction> = Vec::new();
    for (index, message) in messages.into_iter().enumerate() {
        let now: Instant = start + Duration::from_millis(100 * (index as u64 + 1));
        let output;
        (screen, output) = apply_screen_message(screen, message, now, "14:20:31");
        terminal.write(&output.text);
        actions.push(output.action);
    }
    terminal.write(&close(screen));
    Transcript {
        viewport: terminal.viewport(),
        scrollback: terminal.lines(),
        actions,
    }
}

/// What is wrong with every session's layout at `size`: a header printed other than once, more rows than the terminal, or a row as wide as it.
pub(super) fn layout_problems((columns, rows): (usize, usize)) -> Vec<String> {
    let sessions: [Vec<ScreenMessage>; 5] = [
        connecting(),
        failed_compile(),
        signing_in(),
        ready(),
        interrupted(),
    ];
    sessions
        .into_iter()
        .flat_map(|messages| {
            let transcript: Transcript = play(Some((columns, rows)), messages, close_interrupted);
            let headers: usize = transcript
                .scrollback
                .iter()
                .filter(|line| line.starts_with("sqb dev  shop"))
                .count();
            let wide = transcript
                .scrollback
                .iter()
                .filter(|line| cell_len(line) >= columns)
                .map(|line| format!("{columns}x{rows}: too wide: {line}"));
            [
                (headers != 1).then(|| format!("{columns}x{rows}: header printed {headers} times")),
                (transcript.viewport.len() > rows)
                    .then(|| format!("{columns}x{rows}: {} rows shown", transcript.viewport.len())),
            ]
            .into_iter()
            .flatten()
            .chain(wide)
            .collect::<Vec<String>>()
        })
        .collect()
}
