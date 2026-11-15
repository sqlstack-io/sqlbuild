use std::time::Duration;

use crate::cli_text::models::CliStyle;
use crate::screen_text::models::{
    EventLine, EventMark, Hotkey, PhaseLine, PhaseState, ReadyCard, ScreenFacts, Segment, Tone,
    WarehouseState,
};

/// Uncoloured facts without links, `columns` wide, rows indented by two spaces.
pub(super) fn plain_facts(columns: usize) -> ScreenFacts {
    ScreenFacts {
        columns,
        style: CliStyle { use_color: false },
        links: false,
        indent: 2,
    }
}

pub(super) fn millis(value: u64) -> Option<Duration> {
    Some(Duration::from_millis(value))
}

pub(super) fn phase(
    state: PhaseState,
    label: &str,
    detail: &str,
    elapsed: Option<Duration>,
) -> PhaseLine {
    PhaseLine {
        state,
        label: label.to_owned(),
        detail: detail.to_owned(),
        elapsed,
    }
}

/// The `sqb dev` checklist while discovery runs, with one detail too wide for 80 columns.
pub(super) fn starting_checklist() -> Vec<PhaseLine> {
    vec![
        phase(
            PhaseState::Ok,
            "Control store",
            "sqlite \u{b7} target/dev/control.db",
            millis(100),
        ),
        phase(PhaseState::Ok, "API", "127.0.0.1:8000", millis(240)),
        phase(
            PhaseState::Ok,
            "Scheduler",
            "41 triggers paused",
            millis(120),
        ),
        phase(
            PhaseState::Running { frame: 2 },
            "Discover project",
            "Python nodes 287/412 \u{b7} models/marts/finance/fct_revenue_by_region.sql",
            millis(24_650),
        ),
        phase(PhaseState::Pending, "Compile snapshot", "", None),
        phase(PhaseState::Pending, "Publish", "", None),
        phase(PhaseState::Pending, "Warehouse", "", None),
    ]
}

pub(super) fn ready_card(warehouse: WarehouseState) -> ReadyCard {
    ReadyCard {
        version: "0.231.0".to_owned(),
        ready_in: Duration::from_millis(43_120),
        ui_url: "http://127.0.0.1:8000".to_owned(),
        location:
            "dev \u{b7} gen 1 \u{b7} 2280 models \u{b7} 412 python \u{b7} 85 jobs \u{b7} 7 groups"
                .to_owned(),
        adapter: "snowflake".to_owned(),
        warehouse,
        triggers: "41 paused".to_owned(),
        watching: "project/".to_owned(),
    }
}

pub(super) fn event(
    mark: EventMark,
    title: &str,
    detail: &str,
    elapsed: Option<Duration>,
) -> EventLine {
    EventLine {
        clock: "09:41:03".to_owned(),
        mark,
        title: title.to_owned(),
        detail: detail.to_owned(),
        elapsed,
        notes: Vec::new(),
    }
}

/// A failed generation with its clickable location and the retry hint.
pub(super) fn failed_generation() -> EventLine {
    EventLine {
        notes: vec![
            vec![
                Segment {
                    text: "models/finance/fct_revenue.sql:42:7".to_owned(),
                    tone: Tone::Muted,
                    link: Some("file:///work/shop/models/finance/fct_revenue.sql".to_owned()),
                },
                Segment {
                    text: "  unknown column net_amount_usd".to_owned(),
                    tone: Tone::Plain,
                    link: None,
                },
            ],
            vec![Segment {
                text: "r retry \u{b7} l full log".to_owned(),
                tone: Tone::Muted,
                link: None,
            }],
        ],
        ..event(
            EventMark::Failed,
            "gen 4 failed \u{b7} still serving gen 3",
            "",
            None,
        )
    }
}

pub(super) fn hotkeys() -> Vec<Hotkey> {
    [
        ('o', "open UI"),
        ('r', "recompile"),
        ('s', "retry warehouse"),
        ('l', "logs"),
        ('q', "quit"),
    ]
    .into_iter()
    .map(|(key, label)| Hotkey {
        key,
        label: label.to_owned(),
    })
    .collect()
}

/// `row` without its style and hyperlink escapes.
pub(super) fn visible(row: &str) -> String {
    let mut text: String = String::new();
    let mut chars = row.chars().peekable();
    while let Some(character) = chars.next() {
        if character != '\u{1b}' {
            text.push(character);
            continue;
        }
        match chars.next() {
            Some('[') => {
                for code in chars.by_ref() {
                    if code.is_ascii_alphabetic() {
                        break;
                    }
                }
            }
            Some(']') => {
                while let Some(code) = chars.next() {
                    if code == '\u{1b}' && chars.peek() == Some(&'\\') {
                        chars.next();
                        break;
                    }
                }
            }
            _ => {}
        }
    }
    text
}
