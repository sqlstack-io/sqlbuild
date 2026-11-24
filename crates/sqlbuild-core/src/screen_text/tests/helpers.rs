use std::time::Duration;

use crate::cli_text::models::CliStyle;
use crate::screen_text::main::render_dev_header::render_dev_header;
use crate::screen_text::main::render_event_line::render_event_line;
use crate::screen_text::main::render_key_line::render_key_line;
use crate::screen_text::main::render_log_line::render_log_line;
use crate::screen_text::main::render_problem::render_problem;
use crate::screen_text::main::render_prompt::render_prompt;
use crate::screen_text::main::render_ready_box::render_ready_box;
use crate::screen_text::main::render_result::render_result;
use crate::screen_text::main::render_step_line::render_step_line;
use crate::screen_text::main::visible_cells::visible_cells;
use crate::screen_text::models::{
    BoxRow, DevHeader, EventLine, EventMark, Hotkey, PhaseState, Problem, ProblemLevel,
    PromptBlock, ResultLine, ScreenFacts, Segment, StepLine, Tone, TreeEntry, TreeGroup, TreeRow,
};

/// Uncoloured facts without links, `columns` wide and 24 rows tall.
pub(super) fn plain_facts(columns: usize) -> ScreenFacts {
    ScreenFacts {
        columns,
        rows: 24,
        style: CliStyle { use_color: false },
        links: false,
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

pub(super) fn step(
    state: PhaseState,
    label: &str,
    elapsed: Option<Duration>,
    detail: &str,
) -> StepLine {
    StepLine {
        state,
        label: label.to_owned(),
        elapsed,
        detail: detail.to_owned(),
    }
}

pub(super) fn plain(text: &str) -> Segment {
    Segment {
        text: text.to_owned(),
        tone: Tone::Plain,
        link: None,
    }
}

/// The removed-key error the mock-up shows, with a long explanation.
pub(super) fn removed_key() -> Problem {
    Problem {
        level: ProblemLevel::Error,
        title: "sqlbuild_project.toml".to_owned(),
        code: "D001".to_owned(),
        at: vec![plain("sqlbuild_project.toml:23:1")],
        why: "dev.auto_build_on_save was removed in sqb 0.250.0. sqb dev no longer builds on \
              every save by default, and this key is no longer read."
            .to_owned(),
        fix: "replace it with dev.on_save = \"build\" to keep building on save, or delete the \
              line to get the new default, dev.on_save = \"compile\""
            .to_owned(),
    }
}

/// The Snowflake Warehouse row of the ready box.
pub(super) fn warehouse_row() -> BoxRow {
    BoxRow {
        label: "Warehouse".to_owned(),
        parts: [
            "snowflake",
            "account xy12345",
            "auth browser sign-in (OAuth)",
            "role TRANSFORMER",
            "warehouse TRANSFORM_XS",
        ]
        .map(str::to_owned)
        .to_vec(),
        link: None,
    }
}

pub(super) fn ui_row() -> BoxRow {
    BoxRow {
        label: "UI".to_owned(),
        parts: vec!["http://127.0.0.1:8640".to_owned()],
        link: Some("http://127.0.0.1:8640".to_owned()),
    }
}

pub(super) fn run_started() -> EventLine {
    EventLine {
        clock: "14:20:31".to_owned(),
        mark: EventMark::Started,
        title: "run r_8f3a21c0 shop_job".to_owned(),
        detail: "started manually by Sam Lee from the jobs page of the web UI".to_owned(),
        elapsed: None,
        notes: Vec::new(),
    }
}

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
                    ("at", "sql/models/finance/fct_revenue.sql:42:7", Tone::Muted),
                    ("error", "unknown column net_amount_usd", Tone::Error),
                ],
            ),
        ],
        limit: 2,
        flag: "--verbose".to_owned(),
    }
}

/// Every row any renderer draws at `facts`, with whether it is live (cut) rather than lasting (wrapped).
pub(super) fn every_row(facts: &ScreenFacts) -> Vec<(bool, String)> {
    let long: &str = "Python nodes 287/412 \u{b7} sql/models/marts/finance/fct_revenue_by_region.sql \
                      \u{b7} a_very_long_identifier_without_any_spaces_in_it_at_all_whatsoever";
    let mut rows: Vec<(bool, String)> = Vec::new();
    for state in [
        PhaseState::Pending,
        PhaseState::Running { frame: 3 },
        PhaseState::Ok,
        PhaseState::Fail,
    ] {
        let line: StepLine = step(state, "Connecting warehouse", millis(24_650), long);
        rows.extend(
            render_step_line(&line, facts, false)
                .into_iter()
                .map(|row| (true, row)),
        );
        rows.extend(
            render_step_line(&line, facts, true)
                .into_iter()
                .map(|row| (false, row)),
        );
    }
    rows.push((
        true,
        render_key_line(
            &[('o', "open UI"), ('l', "full log"), ('q', "quit")].map(|(key, label)| Hotkey {
                key,
                label: label.to_owned(),
            }),
            facts,
        ),
    ));
    rows.extend(
        render_prompt(
            &PromptBlock {
                title: "snowflake needs you to sign in".to_owned(),
                text: format!(
                    "Go to https://example.com/authorize?{} to sign in\n",
                    "x".repeat(150)
                ),
                waited: Duration::from_secs(24),
            },
            facts,
        )
        .into_iter()
        .map(|row| (true, row)),
    );
    let lasting: Vec<String> = [
        render_dev_header(
            &DevHeader {
                version: "0.256.0".to_owned(),
                project: "shop".to_owned(),
                target: "dev".to_owned(),
            },
            facts,
        ),
        render_problem(&removed_key(), facts),
        render_ready_box(&[ui_row(), warehouse_row()], facts),
        render_event_line(&run_started(), facts),
        render_log_line(long, facts),
        render_result(
            &ResultLine {
                failed: true,
                title: "sqb dev did not start".to_owned(),
                detail: "could not connect to the warehouse".to_owned(),
                elapsed: millis(14_700),
                next: vec![(
                    "sqb debug".to_owned(),
                    "test the connection on its own".to_owned(),
                )],
            },
            facts,
        ),
    ]
    .concat();
    rows.extend(lasting.into_iter().map(|row| (false, row)));
    rows
}

/// Rows wider than they may be: live rows one cell short of the terminal, lasting rows too unless the terminal is narrower than their minimum wrap.
pub(super) fn rows_too_wide(columns: usize, facts: fn(usize) -> ScreenFacts) -> Vec<String> {
    every_row(&facts(columns))
        .into_iter()
        .filter(|(live, row)| {
            let cells: usize = visible_cells(row);
            cells > columns.saturating_sub(1).max(1) && (*live || columns >= 60)
        })
        .map(|(_, row)| format!("{columns}: {row}"))
        .collect()
}
