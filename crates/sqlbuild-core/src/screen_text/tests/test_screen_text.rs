use std::time::Duration;

use crate::cli_text::models::CliStyle;
use crate::screen_text::main::format_elapsed::format_elapsed;
use crate::screen_text::main::format_plain_line::format_plain_line;
use crate::screen_text::main::redraw_rows::redraw_rows;
use crate::screen_text::main::render_dev_header::render_dev_header;
use crate::screen_text::main::render_event_line::render_event_line;
use crate::screen_text::main::render_frame_rule::render_frame_rule;
use crate::screen_text::main::render_hotkey_bar::render_hotkey_bar;
use crate::screen_text::main::render_phase_lines::render_phase_lines;
use crate::screen_text::main::render_ready_card::render_ready_card;
use crate::screen_text::main::render_tree_group::render_tree_group;
use crate::screen_text::models::{
    DevHeader, EventMark, PhaseLayout, PhaseState, ScreenFacts, Tone, TreeEntry, TreeGroup,
    TreeRow, WarehouseState,
};
use crate::screen_text::tests::helpers::{
    event, failed_generation, hotkeys, millis, phase, plain_facts, ready_card, starting_checklist,
    visible,
};
use crate::screen_text::tests::test_types::{
    ElapsedTestCase, PhaseLinesTestCase, ReadyCardTestCase, TreeGroupTestCase,
};
use crate::text::main::cell_len::cell_len;

fn connected() -> WarehouseState {
    WarehouseState::Connected {
        detail: "ANALYST \u{b7} DEVELOPER_ROLE @ DEV_ANALYST_WAREHOUSE_LARGE".to_owned(),
    }
}

fn not_connected() -> WarehouseState {
    WarehouseState::NotConnected {
        reason: "sign-in skipped".to_owned(),
    }
}

fn semantic_group(total: usize) -> TreeGroup {
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

#[test]
fn given_phase_lines_when_rendering_then_columns_align_and_rows_fit() {
    let test_cases = [
        PhaseLinesTestCase {
            description: "dev checklist at 80 columns cuts the widest detail and keeps the times",
            lines: starting_checklist(),
            layout: PhaseLayout::TimeLast,
            facts: plain_facts(80),
            expected_rows: vec![
                "  ✓ Control store        sqlite · target/dev/control.db                     0.1s",
                "  ✓ API                  127.0.0.1:8000                                     0.2s",
                "  ✓ Scheduler            41 triggers paused                                 0.1s",
                "  ⠹ Discover project     Python nodes 287/412 · models/marts/finance/f…    24.6s",
                "  · Compile snapshot",
                "  · Publish",
                "  · Warehouse",
            ],
        },
        PhaseLinesTestCase {
            description: "dev checklist at 120 columns shows every detail in full",
            lines: starting_checklist(),
            layout: PhaseLayout::TimeLast,
            facts: plain_facts(120),
            expected_rows: vec![
                "  ✓ Control store        sqlite · target/dev/control.db                                            0.1s",
                "  ✓ API                  127.0.0.1:8000                                                            0.2s",
                "  ✓ Scheduler            41 triggers paused                                                        0.1s",
                "  ⠹ Discover project     Python nodes 287/412 · models/marts/finance/fct_revenue_by_region.sql    24.6s",
                "  · Compile snapshot",
                "  · Publish",
                "  · Warehouse",
            ],
        },
        PhaseLinesTestCase {
            description: "compile phases put the time beside the label and the detail after it",
            lines: vec![
                phase(PhaseState::Ok, "Discovered project", "", millis(38_800)),
                phase(
                    PhaseState::Ok,
                    "Evaluated rules",
                    "built-in 3.9s \u{b7} custom 18.1s",
                    millis(18_600),
                ),
                phase(
                    PhaseState::Warn,
                    "Inspected cursor bounds",
                    "5 of 5",
                    millis(100),
                ),
                phase(
                    PhaseState::Fail,
                    "Compiled models",
                    "3 errors",
                    millis(1_003_400),
                ),
            ],
            layout: PhaseLayout::TimeFirst,
            facts: ScreenFacts {
                indent: 0,
                ..plain_facts(80)
            },
            expected_rows: vec![
                "✓ Discovered project         38.8s",
                "✓ Evaluated rules            18.6s   built-in 3.9s · custom 18.1s",
                "⚠ Inspected cursor bounds     0.1s   5 of 5",
                "✗ Compiled models          16m 43s   3 errors",
            ],
        },
    ];

    for test_case in test_cases {
        let rows: Vec<String> =
            render_phase_lines(&test_case.lines, test_case.layout, &test_case.facts);

        assert_eq!(rows, test_case.expected_rows, "{}", test_case.description);
    }
}

#[test]
fn given_ready_card_when_rendering_then_rows_match_the_screen() {
    let test_cases = [
        ReadyCardTestCase {
            description: "connected warehouse at 80 columns",
            card: ready_card(connected()),
            facts: plain_facts(80),
            expected_rows: vec![
                "  sqlbuild dev  v0.231.0   ready in 43.1s",
                "",
                "  ➜  UI          http://127.0.0.1:8000",
                "  ➜  Location    dev · gen 1 · 2280 models · 412 python · 85 jobs · 7 groups",
                "  ➜  Warehouse   ● snowflake · ANALYST · DEVELOPER_ROLE @ DEV_ANALYST_WAREHOUSE…",
                "  ➜  Triggers    41 paused",
                "  ➜  Watching    project/",
            ],
        },
        ReadyCardTestCase {
            description: "connected warehouse at 120 columns",
            card: ready_card(connected()),
            facts: plain_facts(120),
            expected_rows: vec![
                "  sqlbuild dev  v0.231.0   ready in 43.1s",
                "",
                "  ➜  UI          http://127.0.0.1:8000",
                "  ➜  Location    dev · gen 1 · 2280 models · 412 python · 85 jobs · 7 groups",
                "  ➜  Warehouse   ● snowflake · ANALYST · DEVELOPER_ROLE @ DEV_ANALYST_WAREHOUSE_LARGE",
                "  ➜  Triggers    41 paused",
                "  ➜  Watching    project/",
            ],
        },
        ReadyCardTestCase {
            description: "unavailable warehouse says nothing will prompt and how to retry",
            card: ready_card(not_connected()),
            facts: plain_facts(80),
            expected_rows: vec![
                "  sqlbuild dev  v0.231.0   ready in 43.1s",
                "",
                "  ➜  UI          http://127.0.0.1:8000",
                "  ➜  Location    dev · gen 1 · 2280 models · 412 python · 85 jobs · 7 groups",
                "  ➜  Warehouse   ○ not connected · sign-in skipped · press s to retry",
                "  ➜  Triggers    41 paused",
                "  ➜  Watching    project/",
            ],
        },
        ReadyCardTestCase {
            description: "a failed connection shows its error",
            card: ready_card(WarehouseState::Failed {
                message: "authentication timed out".to_owned(),
            }),
            facts: plain_facts(80),
            expected_rows: vec![
                "  sqlbuild dev  v0.231.0   ready in 43.1s",
                "",
                "  ➜  UI          http://127.0.0.1:8000",
                "  ➜  Location    dev · gen 1 · 2280 models · 412 python · 85 jobs · 7 groups",
                "  ➜  Warehouse   ● snowflake · authentication timed out · press s to retry",
                "  ➜  Triggers    41 paused",
                "  ➜  Watching    project/",
            ],
        },
    ];

    for test_case in test_cases {
        let rows: Vec<String> = render_ready_card(&test_case.card, &test_case.facts);

        assert_eq!(rows, test_case.expected_rows, "{}", test_case.description);
    }
}

#[test]
fn given_unavailable_warehouse_when_colouring_then_dot_is_amber_not_green() {
    let facts: ScreenFacts = ScreenFacts {
        style: CliStyle { use_color: true },
        ..plain_facts(80)
    };

    let unavailable: String = render_ready_card(&ready_card(not_connected()), &facts)[4].clone();
    let available: String = render_ready_card(&ready_card(connected()), &facts)[4].clone();

    assert!(
        unavailable.contains("\u{1b}[33m\u{25cb}\u{1b}[0m"),
        "{unavailable:?}"
    );
    assert!(
        available.contains("\u{1b}[32m\u{25cf}\u{1b}[0m"),
        "{available:?}"
    );
}

#[test]
fn given_events_when_rendering_then_one_line_each_with_notes_below() {
    let events = [
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
    ];
    let expected: [(usize, Vec<&str>); 2] = [
        (
            80,
            vec![
                "  09:41:03  ~ models/orders/int_orders.sql",
                "  09:41:03  ~ python/exports/partner_feed/fulfilment_partner_daily_export_node.…",
                "  09:41:03  ✓ gen 2 published                 1 model changed      1.4s",
                "  09:41:03  ↻ location restarted              Python changed       3.9s",
                "  09:41:03  ✗ gen 4 failed · still serving gen 3",
                "            models/finance/fct_revenue.sql:42:7  unknown column net_amount_usd",
                "            r retry · l full log",
                "  09:41:03  ▶ run nightly_group               started from UI",
                "  09:41:03  ✓ run nightly_group               212 nodes         16m 43s",
            ],
        ),
        (
            120,
            vec![
                "  09:41:03  ~ models/orders/int_orders.sql",
                "  09:41:03  ~ python/exports/partner_feed/fulfilment_partner_daily_export_node.py",
                "  09:41:03  ✓ gen 2 published                 1 model changed      1.4s",
                "  09:41:03  ↻ location restarted              Python changed       3.9s",
                "  09:41:03  ✗ gen 4 failed · still serving gen 3",
                "            models/finance/fct_revenue.sql:42:7  unknown column net_amount_usd",
                "            r retry · l full log",
                "  09:41:03  ▶ run nightly_group               started from UI",
                "  09:41:03  ✓ run nightly_group               212 nodes         16m 43s",
            ],
        ),
    ];

    for (columns, expected_rows) in expected {
        let rows: Vec<String> = events
            .iter()
            .flat_map(|line| render_event_line(line, &plain_facts(columns)))
            .collect();

        assert_eq!(rows, expected_rows, "{columns} columns");
    }
}

#[test]
fn given_links_when_rendering_failed_generation_then_only_the_location_is_a_hyperlink() {
    let facts: ScreenFacts = ScreenFacts {
        links: true,
        ..plain_facts(80)
    };

    let rows: Vec<String> = render_event_line(&failed_generation(), &facts);

    assert_eq!(
        rows[1],
        "            \u{1b}]8;;file:///work/shop/models/finance/fct_revenue.sql\u{1b}\\models/finance/fct_revenue.sql:42:7\u{1b}]8;;\u{1b}\\  unknown column net_amount_usd"
    );
    assert_eq!(
        visible(&rows[1]).trim_start(),
        "models/finance/fct_revenue.sql:42:7  unknown column net_amount_usd"
    );
}

#[test]
fn given_tree_group_when_rendering_then_rows_are_bounded_with_overflow_line() {
    let test_cases = [
        TreeGroupTestCase {
            description: "two of five entries shown, overflow names the flag",
            group: semantic_group(5),
            facts: ScreenFacts {
                indent: 0,
                ..plain_facts(120)
            },
            expected_rows: vec![
                "Partial semantic checks (5)",
                "├── orders__mart_v__fulfilment_monitoring",
                "│   ├── reason  reads open input orders__mart__fulfilment_monitoring",
                "│   └── fix     declare the input shape, then sqb compile --no-cache",
                "├── fct_revenue",
                "│   ├── at     models/finance/fct_revenue.sql:42:7",
                "│   └── error  unknown column net_amount_usd",
                "└── … 3 more (--verbose)",
            ],
        },
        TreeGroupTestCase {
            description: "every entry shown ends on the last branch and cuts long rows at 60",
            group: semantic_group(2),
            facts: ScreenFacts {
                indent: 0,
                ..plain_facts(60)
            },
            expected_rows: vec![
                "Partial semantic checks (2)",
                "├── orders__mart_v__fulfilment_monitoring",
                "│   ├── reason  reads open input orders__mart__fulfilment_m…",
                "│   └── fix     declare the input shape, then sqb compile -…",
                "└── fct_revenue",
                "    ├── at     models/finance/fct_revenue.sql:42:7",
                "    └── error  unknown column net_amount_usd",
            ],
        },
    ];

    for test_case in test_cases {
        let rows: Vec<String> = render_tree_group(&test_case.group, &test_case.facts);

        assert_eq!(rows, test_case.expected_rows, "{}", test_case.description);
    }
}

#[test]
fn given_header_rule_and_hotkeys_when_rendering_then_match_the_screen() {
    let facts: ScreenFacts = plain_facts(80);
    let header: DevHeader = DevHeader {
        version: "0.231.0".to_owned(),
        project: "shop".to_owned(),
        target: "dev".to_owned(),
    };

    let rows: Vec<String> = vec![
        render_dev_header(&header, &facts),
        render_frame_rule(Some("snowflake"), &facts),
        render_frame_rule(None, &facts),
        render_hotkey_bar(&hotkeys(), &facts),
        render_hotkey_bar(&hotkeys(), &plain_facts(40)),
    ];

    assert_eq!(
        rows,
        vec![
            "  sqlbuild dev  v0.231.0 · shop · target dev",
            "  ── snowflake ─────────────────────────────────────────────────────────────────",
            "  ──────────────────────────────────────────────────────────────────────────────",
            "  o open UI · r recompile · s retry warehouse · l logs · q quit",
            "  o open UI · r recompile · s retry war…",
        ]
    );
}

#[test]
fn given_plain_mode_when_formatting_then_lines_are_stable_and_untimed_lines_have_no_suffix() {
    let lines: Vec<String> = vec![
        format_plain_line("control store ready", millis(100)),
        format_plain_line("location 3f1c9a2e (target dev)", None),
        format_plain_line("discovered 412 python nodes, 2280 models", millis(38_200)),
        format_plain_line("published gen 1 b429729b", millis(1_700)),
        format_plain_line("ready http://127.0.0.1:8000", None),
    ];

    assert_eq!(
        lines,
        vec![
            "sqlbuild dev: control store ready (0.1s)",
            "sqlbuild dev: location 3f1c9a2e (target dev)",
            "sqlbuild dev: discovered 412 python nodes, 2280 models (38.2s)",
            "sqlbuild dev: published gen 1 b429729b (1.7s)",
            "sqlbuild dev: ready http://127.0.0.1:8000",
        ]
    );
}

#[test]
fn given_durations_when_formatting_elapsed_then_units_change_at_minute_and_hour() {
    let test_cases = [
        ElapsedTestCase {
            description: "zero",
            elapsed: Duration::ZERO,
            expected_text: "0.0s",
        },
        ElapsedTestCase {
            description: "tenths truncate rather than round",
            elapsed: Duration::from_millis(59_990),
            expected_text: "59.9s",
        },
        ElapsedTestCase {
            description: "a full minute switches to minutes and seconds",
            elapsed: Duration::from_secs(60),
            expected_text: "1m 0s",
        },
        ElapsedTestCase {
            description: "just under an hour",
            elapsed: Duration::from_secs(3599),
            expected_text: "59m 59s",
        },
        ElapsedTestCase {
            description: "hours drop the seconds",
            elapsed: Duration::from_secs(7_530),
            expected_text: "2h 5m",
        },
    ];

    for test_case in test_cases {
        assert_eq!(
            format_elapsed(test_case.elapsed),
            test_case.expected_text,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_previous_rows_when_redrawing_then_moves_up_clears_and_draws_each_row_once() {
    assert_eq!(
        redraw_rows(3, &["a".to_owned(), "b".to_owned()]),
        "\u{1b}[3F\u{1b}[Ja\nb\n"
    );
    assert_eq!(redraw_rows(0, &["a".to_owned()]), "\u{1b}[Ja\n");
}

#[test]
fn given_any_width_when_rendering_every_piece_then_no_row_is_wider_than_the_screen() {
    for columns in 8..=130 {
        for colour in [false, true] {
            let facts: ScreenFacts = ScreenFacts {
                style: CliStyle { use_color: colour },
                links: colour,
                ..plain_facts(columns)
            };
            let mut rows: Vec<String> =
                render_phase_lines(&starting_checklist(), PhaseLayout::TimeLast, &facts);
            rows.extend(render_phase_lines(
                &starting_checklist(),
                PhaseLayout::TimeFirst,
                &facts,
            ));
            rows.extend(render_ready_card(&ready_card(connected()), &facts));
            rows.extend(render_ready_card(&ready_card(not_connected()), &facts));
            rows.extend(render_event_line(&failed_generation(), &facts));
            rows.extend(render_event_line(
                &event(
                    EventMark::Succeeded,
                    "gen 2 published",
                    "1 model changed",
                    millis(1_400),
                ),
                &facts,
            ));
            rows.extend(render_tree_group(&semantic_group(5), &facts));
            rows.push(render_frame_rule(Some("snowflake"), &facts));
            rows.push(render_hotkey_bar(&hotkeys(), &facts));

            for row in rows {
                let width: usize = cell_len(&visible(&row));
                assert!(
                    width <= columns,
                    "{columns} columns, colour {colour}: {row:?} is {width} wide"
                );
            }
        }
    }
}
