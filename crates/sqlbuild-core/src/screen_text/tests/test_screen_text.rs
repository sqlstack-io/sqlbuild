use std::time::Duration;

use crate::screen_text::_helpers::rows::elapsed_text;
use crate::screen_text::main::format_plain_line::format_plain_line;
use crate::screen_text::main::redraw_rows::redraw_rows;
use crate::screen_text::main::render_event_line::render_event_line;
use crate::screen_text::main::render_phase_lines::render_phase_lines;
use crate::screen_text::main::render_ready_card::render_ready_card;
use crate::screen_text::main::render_tree_group::render_tree_group;
use crate::screen_text::models::{PhaseLayout, PhaseState, ReadyCard, ScreenFacts, WarehouseState};
use crate::screen_text::tests::helpers::{
    chrome_rows, colour_facts, connected, edited_events, millis, not_connected, phase, plain_facts,
    ready_card, rows_wider_than_screen, semantic_group, starting_checklist,
};
use crate::screen_text::tests::test_types::{
    ChromeTestCase, DotTestCase, ElapsedTestCase, EventsTestCase, PhaseLinesTestCase,
    PlainLineTestCase, ReadyCardTestCase, RedrawTestCase, TreeGroupTestCase, WidthTestCase,
};

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
            description: "discovered sets add a row after the watched paths",
            card: ReadyCard {
                discovered: Some(
                    "orders_plan 27 items \u{b7} returns_plan \u{2717} ssh timeout (using last good, 11 items)"
                        .to_owned(),
                ),
                ..ready_card(connected())
            },
            facts: plain_facts(120),
            expected_rows: vec![
                "  sqlbuild dev  v0.231.0   ready in 43.1s",
                "",
                "  ➜  UI          http://127.0.0.1:8000",
                "  ➜  Location    dev · gen 1 · 2280 models · 412 python · 85 jobs · 7 groups",
                "  ➜  Warehouse   ● snowflake · ANALYST · DEVELOPER_ROLE @ DEV_ANALYST_WAREHOUSE_LARGE",
                "  ➜  Triggers    41 paused",
                "  ➜  Watching    project/",
                "  ➜  Discovered  orders_plan 27 items · returns_plan ✗ ssh timeout (using last good, 11 items)",
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
            elapsed_text(test_case.elapsed),
            test_case.expected_text,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_a_warehouse_state_when_colouring_the_card_then_the_dot_matches_it() {
    let test_cases = [
        DotTestCase {
            description: "a skipped sign-in is an amber hollow dot, never green",
            warehouse: not_connected(),
            expected_dot: "\u{1b}[33m\u{25cb}\u{1b}[0m",
        },
        DotTestCase {
            description: "a connected warehouse is a green filled dot",
            warehouse: connected(),
            expected_dot: "\u{1b}[32m\u{25cf}\u{1b}[0m",
        },
    ];

    for test_case in test_cases {
        let rows: Vec<String> =
            render_ready_card(&ready_card(test_case.warehouse), &colour_facts(80));

        assert!(
            rows[4].contains(test_case.expected_dot),
            "{}: {:?}",
            test_case.description,
            rows[4]
        );
    }
}

#[test]
fn given_events_when_rendering_then_one_line_each_with_notes_below() {
    let test_cases = [
        EventsTestCase {
            description: "at 80 columns the long Python path is cut with an ellipsis",
            facts: plain_facts(80),
            expected_rows: vec![
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
        },
        EventsTestCase {
            description: "at 120 columns every path is shown whole and timings stay in their column",
            facts: plain_facts(120),
            expected_rows: vec![
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
        },
        EventsTestCase {
            description: "with links only the error location is a hyperlink; the visible text is unchanged",
            facts: ScreenFacts {
                links: true,
                ..plain_facts(80)
            },
            expected_rows: vec![
                "  09:41:03  ~ models/orders/int_orders.sql",
                "  09:41:03  ~ python/exports/partner_feed/fulfilment_partner_daily_export_node.…",
                "  09:41:03  ✓ gen 2 published                 1 model changed      1.4s",
                "  09:41:03  ↻ location restarted              Python changed       3.9s",
                "  09:41:03  ✗ gen 4 failed · still serving gen 3",
                "            \u{1b}]8;;file:///work/shop/models/finance/fct_revenue.sql\u{1b}\\models/finance/fct_revenue.sql:42:7\u{1b}]8;;\u{1b}\\  unknown column net_amount_usd",
                "            r retry · l full log",
                "  09:41:03  ▶ run nightly_group               started from UI",
                "  09:41:03  ✓ run nightly_group               212 nodes         16m 43s",
            ],
        },
    ];

    for test_case in test_cases {
        let rows: Vec<String> = edited_events()
            .iter()
            .flat_map(|line| render_event_line(line, &test_case.facts))
            .collect();

        assert_eq!(rows, test_case.expected_rows, "{}", test_case.description);
    }
}

#[test]
fn given_header_rule_and_hotkeys_when_rendering_then_match_the_screen() {
    let test_cases = [
        ChromeTestCase {
            description: "at 80 columns the header, both rules and the hotkey bar are whole",
            columns: 80,
            expected_rows: vec![
                "  sqlbuild dev  v0.231.0 · shop · target dev",
                "  ── snowflake ─────────────────────────────────────────────────────────────────",
                "  ──────────────────────────────────────────────────────────────────────────────",
                "  o open UI · r recompile · s retry warehouse · l logs · q quit",
            ],
        },
        ChromeTestCase {
            description: "at 40 columns long rows end in an ellipsis and rules shrink to fit",
            columns: 40,
            expected_rows: vec![
                "  sqlbuild dev  v0.231.0 · shop · targe…",
                "  ── snowflake ─────────────────────────",
                "  ──────────────────────────────────────",
                "  o open UI · r recompile · s retry war…",
            ],
        },
    ];

    for test_case in test_cases {
        let rows: Vec<String> = chrome_rows(&plain_facts(test_case.columns));

        assert_eq!(rows, test_case.expected_rows, "{}", test_case.description);
    }
}

#[test]
fn given_plain_mode_when_formatting_then_lines_are_stable_and_untimed_lines_have_no_suffix() {
    let test_cases = [
        PlainLineTestCase {
            description: "a timed step ends with its time in brackets",
            message: "control store ready",
            elapsed: millis(100),
            expected_line: "sqlbuild dev: control store ready (0.1s)",
        },
        PlainLineTestCase {
            description: "an untimed line has no suffix, even when it ends in brackets itself",
            message: "location 3f1c9a2e (target dev)",
            elapsed: None,
            expected_line: "sqlbuild dev: location 3f1c9a2e (target dev)",
        },
        PlainLineTestCase {
            description: "longer steps keep tenths of a second",
            message: "discovered 412 python nodes, 2280 models",
            elapsed: millis(38_200),
            expected_line: "sqlbuild dev: discovered 412 python nodes, 2280 models (38.2s)",
        },
        PlainLineTestCase {
            description: "the ready line carries the URL and no time",
            message: "ready http://127.0.0.1:8000",
            elapsed: None,
            expected_line: "sqlbuild dev: ready http://127.0.0.1:8000",
        },
    ];

    for test_case in test_cases {
        assert_eq!(
            format_plain_line(test_case.message, test_case.elapsed),
            test_case.expected_line,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_previous_rows_when_redrawing_then_moves_up_clears_and_draws_each_row_once() {
    let test_cases = [
        RedrawTestCase {
            description: "three drawn rows move the cursor up three lines before clearing",
            previous: 3,
            rows: vec!["a".to_owned(), "b".to_owned()],
            expected_text: "\u{1b}[3F\u{1b}[Ja\nb\n",
        },
        RedrawTestCase {
            description: "nothing drawn yet clears in place without moving",
            previous: 0,
            rows: vec!["a".to_owned()],
            expected_text: "\u{1b}[Ja\n",
        },
        RedrawTestCase {
            description: "clearing every row leaves no row behind",
            previous: 2,
            rows: Vec::new(),
            expected_text: "\u{1b}[2F\u{1b}[J",
        },
    ];

    for test_case in test_cases {
        assert_eq!(
            redraw_rows(test_case.previous, &test_case.rows),
            test_case.expected_text,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_any_width_when_rendering_every_piece_then_no_row_is_wider_than_the_screen() {
    let test_cases = [
        WidthTestCase {
            description: "uncoloured rows without links fit every width from 8 to 130",
            colour: false,
            expected_too_wide: Vec::new(),
        },
        WidthTestCase {
            description: "colour and hyperlink escapes take no visible width",
            colour: true,
            expected_too_wide: Vec::new(),
        },
    ];

    for test_case in test_cases {
        let too_wide: Vec<String> = (8..=130)
            .flat_map(|columns| rows_wider_than_screen(columns, test_case.colour))
            .collect();

        assert_eq!(
            too_wide, test_case.expected_too_wide,
            "{}",
            test_case.description
        );
    }
}
