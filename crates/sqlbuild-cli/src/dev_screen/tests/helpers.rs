//! Screens at fixed widths, the messages of a session, and a terminal that replays the redraws.

use std::time::{Duration, Instant};

use sqlbuild_core::cli_text::models::CliStyle;
use sqlbuild_core::screen_text::models::{DevHeader, ScreenFacts, WarehouseState};
use sqlbuild_orchestration::dev_events::models::{
    DevEvent, DevPhase, DevStepState, SourcePosition,
};

use crate::dev_screen::_helpers::apply_screen_message::apply_screen_message;
use crate::dev_screen::_helpers::new_dev_screen::new_dev_screen;
use crate::dev_screen::models::{DevScreen, DevStep, ScreenMessage, ScreenOutput, ScreenSetup};
use crate::dev_screen::tests::test_types::Transcript;

const LOCATION_ID: &str = "01a1d7ff-0b3a-7631-ba35-e44d44d072f2";
const CLEAR_BELOW: &str = "\u{1b}[J";
const CURSOR_UP: &str = "\u{1b}[";

/// An uncoloured screen `columns` wide; `None` makes it plain.
pub(super) fn screen(columns: Option<usize>, start: Instant) -> DevScreen {
    new_dev_screen(
        ScreenSetup {
            live: columns.is_some(),
            facts: ScreenFacts {
                columns: columns.unwrap_or(80),
                style: CliStyle { use_color: false },
                links: false,
                indent: 2,
            },
            header: DevHeader {
                version: "0.232.0".to_owned(),
                project: "shop".to_owned(),
                target: "dev".to_owned(),
            },
            watching: "project/".to_owned(),
            project_root: "/work/shop".to_owned(),
            env_file: None,
        },
        start,
    )
}

pub(super) fn step(phase: DevPhase, state: DevStepState, detail: Option<&str>) -> ScreenMessage {
    ScreenMessage::Child(DevEvent::Step {
        phase,
        state,
        detail: detail.map(str::to_owned),
    })
}

/// Startup up to the first publication, each message 100 ms after the last.
pub(super) fn startup() -> Vec<ScreenMessage> {
    vec![
        ScreenMessage::Started {
            step: DevStep::ControlStore,
            detail: String::new(),
        },
        ScreenMessage::Finished {
            step: DevStep::ControlStore,
            detail: "sqlite \u{b7} /home/analyst/projects/shop/.sqlbuild/control.sqlite3"
                .to_owned(),
        },
        ScreenMessage::Location(LOCATION_ID.to_owned()),
        ScreenMessage::Started {
            step: DevStep::Api,
            detail: String::new(),
        },
        ScreenMessage::Finished {
            step: DevStep::Api,
            detail: "127.0.0.1:8000".to_owned(),
        },
        ScreenMessage::Started {
            step: DevStep::Scheduler,
            detail: String::new(),
        },
        ScreenMessage::Finished {
            step: DevStep::Scheduler,
            detail: "41 paused".to_owned(),
        },
        step(DevPhase::Discover, DevStepState::Started, None),
        step(DevPhase::Discover, DevStepState::Finished, None),
        step(DevPhase::Compile, DevStepState::Started, None),
        step(
            DevPhase::Compile,
            DevStepState::Finished,
            Some("412 python nodes, 2280 models"),
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
            detail: "duckdb \u{b7} connecting".to_owned(),
        },
    ]
}

pub(super) fn connected() -> ScreenMessage {
    ScreenMessage::Warehouse {
        adapter: "duckdb".to_owned(),
        state: WarehouseState::Connected {
            detail: "main".to_owned(),
        },
    }
}

pub(super) fn not_connected() -> ScreenMessage {
    ScreenMessage::Warehouse {
        adapter: "snowflake".to_owned(),
        state: WarehouseState::NotConnected {
            reason: "sign-in skipped".to_owned(),
        },
    }
}

/// `messages` after startup.
pub(super) fn after_startup(messages: Vec<ScreenMessage>) -> Vec<ScreenMessage> {
    startup().into_iter().chain(messages).collect()
}

/// Two edits after ready: one publishes, the next fails at a source position; then a sync stops.
pub(super) fn edits() -> Vec<ScreenMessage> {
    vec![
        ScreenMessage::Child(DevEvent::Changed {
            paths: vec!["models/orders/int_orders.sql".to_owned()],
            more: 0,
        }),
        step(DevPhase::Discover, DevStepState::Started, None),
        ScreenMessage::Child(DevEvent::Published {
            generation: 2,
            snapshot_id: "c01d2e3f4a".to_owned(),
            record: serde_json::Value::Null,
        }),
        ScreenMessage::Child(DevEvent::Changed {
            paths: vec!["models/finance/fct_revenue.sql".to_owned()],
            more: 2,
        }),
        step(DevPhase::Discover, DevStepState::Started, None),
        ScreenMessage::Child(DevEvent::Failed {
            generation: 3,
            message: "unknown column net_amount_usd".to_owned(),
            position: Some(SourcePosition {
                path: "models/finance/fct_revenue.sql".to_owned(),
                line: 42,
                column: 7,
            }),
        }),
        ScreenMessage::Child(DevEvent::Warning {
            title: "warehouse sync stopped".to_owned(),
            detail: "5 failed reads in a row: ProgrammingError: insufficient privileges".to_owned(),
        }),
    ]
}

/// A Snowflake-style sign-in prompt, ticks while it waits, a log line, then the connection.
pub(super) fn framed_sign_in() -> Vec<ScreenMessage> {
    vec![
        ScreenMessage::ClientOutput(
            "Initiating login request with your identity provider...\n".to_owned(),
        ),
        ScreenMessage::Tick,
        ScreenMessage::ClientOutput("Enter the URL the OAuth flow redirected you to: ".to_owned()),
        ScreenMessage::Tick,
        ScreenMessage::Log("connection check started".to_owned()),
        connected(),
    ]
}

/// The ready card with `warehouse` on its warehouse row.
pub(super) fn card(warehouse: &str, ready_in: &str) -> Vec<String> {
    vec![
        format!("  sqlbuild dev  v0.232.0   ready in {ready_in}"),
        String::new(),
        "  \u{279c}  UI          http://127.0.0.1:8000".to_owned(),
        "  \u{279c}  Location    dev \u{b7} gen 1 \u{b7} 412 python nodes, 2280 models".to_owned(),
        format!("  \u{279c}  Warehouse   {warehouse}"),
        "  \u{279c}  Triggers    41 paused".to_owned(),
        "  \u{279c}  Watching    project/".to_owned(),
    ]
}

/// The startup checklist, its detail column `width` wide.
pub(super) fn checklist(control_store: &str, width: usize) -> Vec<String> {
    let row = |glyph: &str, label: &str, detail: &str, time: &str| -> String {
        format!("  {glyph} {label:<20} {detail:<width$} {time:>8}")
    };
    vec![
        "  sqlbuild dev  v0.232.0 \u{b7} shop \u{b7} target dev".to_owned(),
        String::new(),
        row("\u{2713}", "Control store", control_store, "0.1s"),
        row("\u{2713}", "API", "127.0.0.1:8000", "0.1s"),
        row("\u{2713}", "Scheduler", "41 paused", "0.1s"),
        row("\u{2713}", "Discover project", "", "0.1s"),
        row(
            "\u{2713}",
            "Compile snapshot",
            "412 python nodes, 2280 models",
            "0.1s",
        ),
        row("\u{2713}", "Publish", "gen 1 \u{b7} b429729b", "0.1s"),
        row("\u{280b}", "Warehouse", "duckdb \u{b7} connecting", "0.0s"),
    ]
}

/// Lines in order, each followed by a newline.
pub(super) fn lines_of<I: IntoIterator<Item = S>, S: Into<String>>(lines: I) -> Vec<String> {
    lines.into_iter().map(Into::into).collect()
}

/// Apply `messages` 100 ms apart at clock `09:41:<index>`, then replay what they wrote.
pub(super) fn play(columns: Option<usize>, messages: Vec<ScreenMessage>) -> Transcript {
    let start: Instant = Instant::now();
    let (_, outputs): (DevScreen, Vec<ScreenOutput>) = messages.into_iter().enumerate().fold(
        (screen(columns, start), Vec::new()),
        |(screen, mut outputs), (index, message)| {
            let now: Instant = start + Duration::from_millis(100 * (index as u64 + 1));
            let (screen, output) =
                apply_screen_message(screen, message, now, &format!("09:41:{:02}", index % 60));
            outputs.push(output);
            (screen, outputs)
        },
    );
    Transcript {
        rows: replay(outputs.iter().map(|output| output.text.as_str())),
        actions: outputs.iter().map(|output| output.action).collect(),
    }
}

/// The rows a terminal shows after `writes`; `ESC[nF ESC[J` erases the last `n` rows.
fn replay<'a>(writes: impl Iterator<Item = &'a str>) -> Vec<String> {
    let text: String = writes
        .flat_map(|write| write.split_inclusive(CLEAR_BELOW))
        .fold(String::new(), |shown, piece| {
            let clears: bool = piece.ends_with(CLEAR_BELOW);
            let piece: &str = piece.trim_end_matches(CLEAR_BELOW);
            let (printed, up): (&str, usize) = piece
                .rsplit_once(CURSOR_UP)
                .filter(|_| clears)
                .map_or((piece, 0), |(printed, rows)| {
                    (printed, rows.trim_end_matches('F').parse().unwrap_or(0))
                });
            let shown: String = shown + printed;
            let keep: usize = shown.split_inclusive('\n').count().saturating_sub(up);
            shown.split_inclusive('\n').take(keep).collect()
        });
    text.split_inclusive('\n')
        .map(|row| row.trim_end_matches('\n').to_owned())
        .collect()
}

/// `count` log lines from `log line <first>`, as the open log shows them.
pub(super) fn log_rows(first: usize, count: usize) -> Vec<String> {
    (first..first + count)
        .map(|index| format!("  log line {index}"))
        .collect()
}

/// Every row that is a log line.
pub(super) fn shown_log(rows: &[String]) -> Vec<String> {
    rows.iter()
        .filter(|row| row.starts_with("  log line "))
        .cloned()
        .collect()
}
