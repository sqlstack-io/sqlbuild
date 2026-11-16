use std::time::Duration;

use regex::Regex;

use crate::cli_text::models::CliStyle;
use crate::screen_text::main::render_dev_header::render_dev_header;
use crate::screen_text::main::render_event_line::render_event_line;
use crate::screen_text::main::render_frame_rule::render_frame_rule;
use crate::screen_text::main::render_hotkey_bar::render_hotkey_bar;
use crate::screen_text::main::render_phase_lines::render_phase_lines;
use crate::screen_text::main::render_ready_card::render_ready_card;
use crate::screen_text::main::render_tree_group::render_tree_group;
use crate::screen_text::models::{
    DevHeader, EventLine, EventMark, Hotkey, PhaseLayout, PhaseLine, PhaseState, ReadyCard,
    ScreenFacts, Segment, Tone, TreeEntry, TreeGroup, TreeRow, WarehouseState,
};
use crate::text::main::cell_len::cell_len;

/// Uncoloured facts without links, `columns` wide, rows indented by two spaces.
pub(super) fn plain_facts(columns: usize) -> ScreenFacts {
    ScreenFacts {
        columns,
        style: CliStyle { use_color: false },
        links: false,
        indent: 2,
    }
}

/// Coloured facts with links, `columns` wide.
pub(super) fn colour_facts(columns: usize) -> ScreenFacts {
    ScreenFacts {
        style: CliStyle { use_color: true },
        links: true,
        ..plain_facts(columns)
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

/// A connected warehouse with a long role and warehouse detail.
pub(super) fn connected() -> WarehouseState {
    WarehouseState::Connected {
        detail: "ANALYST \u{b7} DEVELOPER_ROLE @ DEV_ANALYST_WAREHOUSE_LARGE".to_owned(),
    }
}

/// A warehouse whose sign-in was skipped.
pub(super) fn not_connected() -> WarehouseState {
    WarehouseState::NotConnected {
        reason: "sign-in skipped".to_owned(),
    }
}

/// Two partial semantic checks of `total`, one with a clickable error location.
pub(super) fn semantic_group(total: usize) -> TreeGroup {
    let entry = |name: &str, rows: &[(&str, &str, Tone)]| TreeEntry {
        name: name.to_owned(),
        rows: rows
            .iter()
            .map(|(key, value, tone)| TreeRow {
                key: (*key).to_owned(),
                value: (*value).to_owned(),
                tone: *tone,
                link: (*key == "at").then(|| format!("file:///work/shop/{value}")),
            })
            .collect(),
    };
    TreeGroup {
        title: "Partial semantic checks".to_owned(),
        total,
        entries: vec![
            entry(
                "orders__mart_v__fulfilment_monitoring",
                &[
                    (
                        "reason",
                        "reads open input orders__mart__fulfilment_monitoring",
                        Tone::Plain,
                    ),
                    (
                        "fix",
                        "declare the input shape, then sqb compile --no-cache",
                        Tone::Plain,
                    ),
                ],
            ),
            entry(
                "fct_revenue",
                &[
                    ("at", "models/finance/fct_revenue.sql:42:7", Tone::Muted),
                    ("error", "unknown column net_amount_usd", Tone::Error),
                ],
            ),
        ],
        limit: 2,
        flag: "--verbose".to_owned(),
    }
}

/// The events of one editing session: two changes, a publish, a restart, a failure and a run.
pub(super) fn edited_events() -> Vec<EventLine> {
    vec![
        event(EventMark::Changed, "models/orders/int_orders.sql", "", None),
        event(
            EventMark::Changed,
            "python/exports/partner_feed/fulfilment_partner_daily_export_node.py",
            "",
            None,
        ),
        event(
            EventMark::Succeeded,
            "gen 2 published",
            "1 model changed",
            millis(1_400),
        ),
        event(
            EventMark::Restarted,
            "location restarted",
            "Python changed",
            millis(3_900),
        ),
        failed_generation(),
        event(
            EventMark::Started,
            "run nightly_group",
            "started from UI",
            None,
        ),
        event(
            EventMark::Succeeded,
            "run nightly_group",
            "212 nodes",
            millis(1_003_000),
        ),
    ]
}

/// The header, a titled rule, a plain rule and the hotkey bar.
pub(super) fn chrome_rows(facts: &ScreenFacts) -> Vec<String> {
    let header: DevHeader = DevHeader {
        version: "0.231.0".to_owned(),
        project: "shop".to_owned(),
        target: "dev".to_owned(),
    };
    vec![
        render_dev_header(&header, facts),
        render_frame_rule(Some("snowflake"), facts),
        render_frame_rule(None, facts),
        render_hotkey_bar(&hotkeys(), facts),
    ]
}

/// Every row any screen piece draws at `columns` whose visible text is wider than the screen.
pub(super) fn rows_wider_than_screen(columns: usize, colour: bool) -> Vec<String> {
    let facts: ScreenFacts = ScreenFacts {
        style: CliStyle { use_color: colour },
        links: colour,
        ..plain_facts(columns)
    };
    let escapes: Regex = Regex::new("\u{1b}\\[[0-9;]*[A-Za-z]|\u{1b}\\][^\u{1b}]*\u{1b}\\\\")
        .expect("escape pattern");
    let mut rows: Vec<String> =
        render_phase_lines(&starting_checklist(), PhaseLayout::TimeLast, &facts);
    rows.extend(render_phase_lines(
        &starting_checklist(),
        PhaseLayout::TimeFirst,
        &facts,
    ));
    rows.extend(render_ready_card(&ready_card(connected()), &facts));
    rows.extend(render_ready_card(&ready_card(not_connected()), &facts));
    rows.extend(
        edited_events()
            .iter()
            .flat_map(|line| render_event_line(line, &facts)),
    );
    rows.extend(render_tree_group(&semantic_group(5), &facts));
    rows.extend(chrome_rows(&facts));
    rows.into_iter()
        .map(|row| escapes.replace_all(&row, "").into_owned())
        .filter(|row| cell_len(row) > columns)
        .map(|row| format!("{columns} columns: {row}"))
        .collect()
}
