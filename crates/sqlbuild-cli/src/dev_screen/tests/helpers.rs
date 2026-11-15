//! A screen at a fixed width, a transcript of messages, and a terminal that replays the redraws.

use std::time::{Duration, Instant};

use sqlbuild_core::cli_text::models::CliStyle;
use sqlbuild_core::screen_text::models::{DevHeader, ScreenFacts, WarehouseState};
use sqlbuild_orchestration::dev_events::models::{DevEvent, DevPhase, DevStepState};

use crate::dev_screen::main::apply_screen_message::apply_screen_message;
use crate::dev_screen::main::new_dev_screen::new_dev_screen;
use crate::dev_screen::models::{
    DevScreen, DevStep, ScreenAction, ScreenMessage, ScreenSetup,
};

/// An uncoloured screen `columns` wide; `None` makes it plain.
pub(super) fn screen(columns: Option<usize>, start: Instant) -> DevScreen {
    let mut screen: DevScreen = new_dev_screen(
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
            location: "dev".to_owned(),
            watching: "project/".to_owned(),
            project_root: "/work/shop".to_owned(),
        },
        start,
    );
    screen.ui_url = "http://127.0.0.1:8000".to_owned();
    screen
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
            detail: "sqlite \u{b7} /home/analyst/projects/shop/.sqlbuild/control.sqlite3".to_owned(),
        },
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

/// What a terminal shows after `messages`, and the actions they asked for.
pub(super) struct Transcript {
    pub(super) rows: Vec<String>,
    pub(super) actions: Vec<ScreenAction>,
    pub(super) writes: Vec<String>,
}

/// Apply `messages` 100 ms apart, at clock `09:41:0N`, and replay the output on a terminal.
pub(super) fn play(screen: &mut DevScreen, start: Instant, messages: Vec<ScreenMessage>) -> Transcript {
    let mut terminal: Terminal = Terminal::default();
    let mut actions: Vec<ScreenAction> = Vec::new();
    let mut writes: Vec<String> = Vec::new();
    for (index, message) in messages.into_iter().enumerate() {
        let now: Instant = start + Duration::from_millis(100 * (index as u64 + 1));
        let clock: String = format!("09:41:{:02}", index % 60);
        let output = apply_screen_message(screen, message, now, &clock);
        terminal.write(&output.text);
        actions.push(output.action);
        writes.push(output.text);
    }
    Transcript {
        rows: terminal.rows(),
        actions,
        writes,
    }
}

/// Lines of text with the cursor at the start of a row; understands `ESC[nF` and `ESC[J`.
#[derive(Default)]
struct Terminal {
    lines: Vec<String>,
    cursor: usize,
}

impl Terminal {
    fn write(&mut self, text: &str) {
        let mut rest: &str = text;
        while !rest.is_empty() {
            if let Some(after) = rest.strip_prefix("\u{1b}[J") {
                self.lines.truncate(self.cursor);
                rest = after;
            } else if let Some(after) = rest.strip_prefix("\u{1b}[") {
                let digits: usize = after.find('F').expect("cursor-up sequence");
                let up: usize = after[..digits].parse().expect("row count");
                self.cursor = self.cursor.saturating_sub(up);
                rest = &after[digits + 1..];
            } else {
                let end: usize = rest.find('\u{1b}').unwrap_or(rest.len());
                for character in rest[..end].chars() {
                    if self.lines.len() == self.cursor {
                        self.lines.push(String::new());
                    }
                    if character == '\n' {
                        self.cursor += 1;
                    } else {
                        self.lines[self.cursor].push(character);
                    }
                }
                rest = &rest[end..];
            }
        }
    }

    fn rows(&self) -> Vec<String> {
        self.lines.clone()
    }
}
