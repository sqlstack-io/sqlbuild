use std::time::Duration;

use crate::screen_text::_helpers::rows::elapsed_text;
use crate::screen_text::main::redraw_rows::redraw_rows;
use crate::screen_text::main::render_event_line::render_event_line;
use crate::screen_text::main::render_problem::render_problem;
use crate::screen_text::main::render_prompt::render_prompt;
use crate::screen_text::main::render_ready_box::render_ready_box;
use crate::screen_text::main::render_result::render_result;
use crate::screen_text::main::render_step_line::render_step_line;
use crate::screen_text::main::render_tree_group::render_tree_group;
use crate::screen_text::main::visible_cells::visible_cells;
use crate::screen_text::models::{
    BoxRow, EventLine, EventMark, PhaseState, Problem, ProblemLevel, PromptBlock, ResultLine,
};
use crate::screen_text::tests::helpers::{
    colour_facts, millis, plain, plain_facts, removed_key, rows_too_wide, run_started,
    semantic_group, step, ui_row, warehouse_row,
};
use crate::screen_text::tests::test_types::{
    ElapsedTestCase, EventTestCase, ProblemTestCase, PromptTestCase, ReadyBoxTestCase,
    RedrawTestCase, ResultTestCase, StepLineTestCase, TreeGroupTestCase, VisibleCellsTestCase,
    WidthTestCase,
};

#[test]
fn given_step_lines_when_rendering_then_label_time_and_detail_line_up() {
    let test_cases = [
        StepLineTestCase {
            description: "a finished step has its glyph, label, time and detail in columns",
            line: step(
                PhaseState::Ok,
                "Discovered project",
                millis(1_800),
                "118 models \u{b7} 24 seeds",
            ),
            lasting: true,
            columns: 80,
            expected_rows: vec![
                "\u{2713} Discovered project      1.8s   118 models \u{b7} 24 seeds",
            ],
        },
        StepLineTestCase {
            description: "a running step shows its spinner frame and live time",
            line: step(
                PhaseState::Running { frame: 0 },
                "Connecting warehouse",
                millis(10_300),
                "snowflake",
            ),
            lasting: false,
            columns: 80,
            expected_rows: vec!["\u{280b} Connecting warehouse   10.3s   snowflake"],
        },
        StepLineTestCase {
            description: "a pending step has no time",
            line: step(PhaseState::Pending, "Connect warehouse", None, "waiting"),
            lasting: false,
            columns: 80,
            expected_rows: vec!["\u{b7} Connect warehouse              waiting"],
        },
        StepLineTestCase {
            description: "a live row is cut one cell short of the terminal",
            line: step(
                PhaseState::Ok,
                "Compiled project",
                millis(2_400),
                "0 errors \u{b7} 1 warning \u{b7} 412 python nodes",
            ),
            lasting: false,
            columns: 50,
            expected_rows: vec![
                "\u{2713} Compiled project        2.4s   0 errors \u{b7} 1 wa\u{2026}",
            ],
        },
        StepLineTestCase {
            description: "a lasting row wraps its detail under the detail column",
            line: step(
                PhaseState::Ok,
                "Compiled project",
                millis(2_400),
                "0 errors \u{b7} 1 warning \u{b7} 412 python nodes",
            ),
            lasting: true,
            columns: 60,
            expected_rows: vec![
                "\u{2713} Compiled project        2.4s   0 errors \u{b7} 1 warning \u{b7} 412",
                "                                 python nodes",
            ],
        },
    ];

    for test_case in test_cases {
        assert_eq!(
            render_step_line(
                &test_case.line,
                &plain_facts(test_case.columns),
                test_case.lasting
            ),
            test_case.expected_rows,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_a_problem_when_rendering_then_every_word_is_kept_under_its_label() {
    let test_cases = [
        ProblemTestCase {
            description: "a long error wraps with a hanging indent and its code at the right edge",
            problem: removed_key(),
            columns: 80,
            expected_rows: vec![
                "\u{2717} error  sqlbuild_project.toml                                             D001",
                "  at     sqlbuild_project.toml:23:1",
                "  why    dev.auto_build_on_save was removed in sqb 0.250.0. sqb dev no longer",
                "         builds on every save by default, and this key is no longer read.",
                "  fix    replace it with dev.on_save = \"build\" to keep building on save, or",
                "         delete the line to get the new default, dev.on_save = \"compile\"",
            ],
        },
        ProblemTestCase {
            description: "a warning without a place or fix shows only what it has",
            problem: Problem {
                level: ProblemLevel::Warning,
                title: "relation ANALYTICS.LEGACY.V_ORDERS_OLD".to_owned(),
                code: String::new(),
                at: Vec::new(),
                why: "columns are unavailable".to_owned(),
                fix: String::new(),
            },
            columns: 80,
            expected_rows: vec![
                "\u{26a0} warn   relation ANALYTICS.LEGACY.V_ORDERS_OLD",
                "  why    columns are unavailable",
            ],
        },
        ProblemTestCase {
            description: "a word longer than the line is broken, never cut",
            problem: Problem {
                why: "x".repeat(50),
                ..removed_key()
            },
            columns: 40,
            expected_rows: vec![
                "\u{2717} error  sqlbuild_project.toml     D001",
                "  at     sqlbuild_project.toml:23:1",
                "  why    xxxxxxxxxxxxxxxxxxxxxxxxxxxxxx",
                "         xxxxxxxxxxxxxxxxxxxx",
                "  fix    replace it with dev.on_save =",
                "         \"build\" to keep building on",
                "         save, or delete the line to",
                "         get the new default,",
                "         dev.on_save = \"compile\"",
            ],
        },
    ];

    for test_case in test_cases {
        assert_eq!(
            render_problem(&test_case.problem, &plain_facts(test_case.columns)),
            test_case.expected_rows,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_ready_rows_when_rendering_the_box_then_parts_wrap_between_labelled_lines() {
    let test_cases = [
        ReadyBoxTestCase {
            description: "at 80 columns the Warehouse line wraps onto a second line between parts",
            rows: vec![ui_row(), warehouse_row()],
            columns: 80,
            expected_rows: vec![
                "\u{256d}\u{2500} ready \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{256e}",
                "\u{2502} UI         http://127.0.0.1:8640                                           \u{2502}",
                "\u{2502} Warehouse  snowflake \u{b7} account xy12345 \u{b7} auth browser sign-in (OAuth)      \u{2502}",
                "\u{2502}            role TRANSFORMER \u{b7} warehouse TRANSFORM_XS                       \u{2502}",
                "\u{2570}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{256f}",
            ],
        },
        ReadyBoxTestCase {
            description: "discovered sets add a row, each set after the first wrapping whole",
            rows: vec![
                ui_row(),
                BoxRow {
                    label: "Discovered".to_owned(),
                    parts: vec![
                        "orders_plan 27 items".to_owned(),
                        "returns_plan \u{2717} ssh timeout (using last good, 11 items)".to_owned(),
                    ],
                    link: None,
                },
            ],
            columns: 80,
            expected_rows: vec![
                "\u{256d}\u{2500} ready \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{256e}",
                "\u{2502} UI         http://127.0.0.1:8640                                           \u{2502}",
                "\u{2502} Discovered orders_plan 27 items                                            \u{2502}",
                "\u{2502}            returns_plan \u{2717} ssh timeout (using last good, 11 items)          \u{2502}",
                "\u{2570}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{256f}",
            ],
        },
        ReadyBoxTestCase {
            description: "a part wider than the box starts its own line and wraps by words",
            rows: vec![BoxRow {
                label: "Project".to_owned(),
                parts: vec!["shop".to_owned(), "a b c d e f g h i j".to_owned()],
                link: None,
            }],
            columns: 30,
            expected_rows: vec![
                "\u{256d}\u{2500} ready \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{256e}",
                "\u{2502} Project    shop          \u{2502}",
                "\u{2502}            a b c d e f g \u{2502}",
                "\u{2502}            h i j         \u{2502}",
                "\u{2570}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{256f}",
            ],
        },
    ];

    for test_case in test_cases {
        assert_eq!(
            render_ready_box(&test_case.rows, &plain_facts(test_case.columns)),
            test_case.expected_rows,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_events_when_rendering_then_run_lines_wrap_instead_of_being_cut() {
    let test_cases = [
        EventTestCase {
            description: "a long run line wraps under its text",
            event: run_started(),
            columns: 60,
            expected_rows: vec![
                "14:20:31 \u{25b8} run r_8f3a21c0 shop_job  started manually by Sam",
                "           Lee from the jobs page of the web UI",
            ],
        },
        EventTestCase {
            description: "a failure shows its time and its notes below it",
            event: EventLine {
                clock: "14:24:48".to_owned(),
                mark: EventMark::Failed,
                title: "run r_8f3a21c0 shop_job".to_owned(),
                detail: "failed".to_owned(),
                elapsed: Some(Duration::from_secs(257)),
                notes: vec!["1 failed \u{b7} 5 skipped".to_owned()],
            },
            columns: 80,
            expected_rows: vec![
                "14:24:48 \u{2717} run r_8f3a21c0 shop_job  failed  4m 17s",
                "           1 failed \u{b7} 5 skipped",
            ],
        },
    ];

    for test_case in test_cases {
        assert_eq!(
            render_event_line(&test_case.event, &plain_facts(test_case.columns)),
            test_case.expected_rows,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_a_session_end_when_rendering_then_one_result_line_and_next_commands() {
    let test_cases = [
        ResultTestCase {
            description: "a start-up failure says why, how long, and what to run next",
            result: ResultLine {
                failed: true,
                title: "sqb dev did not start".to_owned(),
                detail: "could not connect to the warehouse".to_owned(),
                elapsed: millis(14_700),
                next: vec![(
                    "sqb debug".to_owned(),
                    "test the connection on its own".to_owned(),
                )],
            },
            columns: 80,
            expected_rows: vec![
                "\u{2717} sqb dev did not start  could not connect to the warehouse   14.7s",
                "Next",
                "  sqb debug                     test the connection on its own",
            ],
        },
        ResultTestCase {
            description: "an interrupted stop is a quiet line",
            result: ResultLine {
                failed: false,
                title: "sqb dev stopped (interrupted)".to_owned(),
                detail: "ran 42m 10s".to_owned(),
                elapsed: millis(1_100),
                next: Vec::new(),
            },
            columns: 80,
            expected_rows: vec!["\u{b7} sqb dev stopped (interrupted)  ran 42m 10s   1.1s"],
        },
    ];

    for test_case in test_cases {
        assert_eq!(
            render_result(&test_case.result, &plain_facts(test_case.columns)),
            test_case.expected_rows,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_a_sign_in_prompt_when_rendering_then_its_url_is_broken_across_rows_not_cut() {
    let test_cases = [PromptTestCase {
        description: "the client's prompt and a long URL keep every character",
        text: "Go to https://xy12345.snowflakecomputing.com/oauth/authorize?client_id=SQLBUILD&state=8f3a21c0\nEnter the URL: ",
        columns: 60,
        expected_rows: vec![
            "\u{25cc} sign in  snowflake needs you to sign in",
            "  Go to https://xy12345.snowflakecomputing.com/oauth/author",
            "  ize?client_id=SQLBUILD&state=8f3a21c0",
            "  Enter the URL:",
            "  wait   0:24 \u{b7} Ctrl+C cancels and stops sqb dev",
            "\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}",
        ],
    }];

    for test_case in test_cases {
        assert_eq!(
            render_prompt(
                &PromptBlock {
                    title: "snowflake needs you to sign in".to_owned(),
                    text: test_case.text.to_owned(),
                    waited: Duration::from_secs(24),
                },
                &plain_facts(test_case.columns)
            ),
            test_case.expected_rows,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_durations_when_formatting_elapsed_then_units_change_at_minute_and_hour() {
    let test_cases = [
        ElapsedTestCase {
            description: "under a minute shows tenths, truncated",
            elapsed: Duration::from_millis(24_690),
            expected_text: "24.6s",
        },
        ElapsedTestCase {
            description: "minutes show two-digit seconds",
            elapsed: Duration::from_secs(62),
            expected_text: "1m 02s",
        },
        ElapsedTestCase {
            description: "hours show two-digit minutes",
            elapsed: Duration::from_secs(7_500),
            expected_text: "2h 05m",
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
fn given_styled_rows_when_measuring_then_escapes_take_no_cells() {
    let test_cases = [
        VisibleCellsTestCase {
            description: "plain text counts its cells",
            row: "\u{2713} ok".to_owned(),
            expected_cells: 4,
        },
        VisibleCellsTestCase {
            description: "colour and hyperlink escapes count nothing",
            row: render_problem(
                &Problem {
                    at: vec![plain("a")],
                    ..removed_key()
                },
                &colour_facts(80),
            )[1]
            .clone(),
            expected_cells: 10,
        },
    ];

    for test_case in test_cases {
        assert_eq!(
            visible_cells(&test_case.row),
            test_case.expected_cells,
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
            description: "uncoloured rows fit every width from 8 to 130",
            facts: plain_facts,
            expected_too_wide: Vec::new(),
        },
        WidthTestCase {
            description: "colour and hyperlink escapes take no visible width",
            facts: colour_facts,
            expected_too_wide: Vec::new(),
        },
    ];

    for test_case in test_cases {
        let too_wide: Vec<String> = (8..=130)
            .flat_map(|columns| rows_too_wide(columns, test_case.facts))
            .collect();

        assert_eq!(
            too_wide, test_case.expected_too_wide,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_tree_group_when_rendering_then_rows_are_bounded_with_overflow_line() {
    let test_cases = [
        TreeGroupTestCase {
            description: "two of five entries shown, overflow names the flag",
            group: semantic_group(5),
            facts: plain_facts(120),
            expected_rows: vec![
                "Partial semantic checks (5)",
                "\u{251c}\u{2500}\u{2500} orders__mart_v__fulfilment_monitoring",
                "\u{2502}   \u{251c}\u{2500}\u{2500} reason  reads open input orders__mart__fulfilment_monitoring",
                "\u{2502}   \u{2514}\u{2500}\u{2500} fix     declare the input shape, then sqb compile --no-cache",
                "\u{251c}\u{2500}\u{2500} fct_revenue",
                "\u{2502}   \u{251c}\u{2500}\u{2500} at     models/finance/fct_revenue.sql:42:7",
                "\u{2502}   \u{2514}\u{2500}\u{2500} error  unknown column net_amount_usd",
                "\u{2514}\u{2500}\u{2500} \u{2026} 3 more (--verbose)",
            ],
        },
        TreeGroupTestCase {
            description: "every entry shown ends on the last branch and cuts long rows at 60",
            group: semantic_group(2),
            facts: plain_facts(60),
            expected_rows: vec![
                "Partial semantic checks (2)",
                "\u{251c}\u{2500}\u{2500} orders__mart_v__fulfilment_monitoring",
                "\u{2502}   \u{251c}\u{2500}\u{2500} reason  reads open input orders__mart__fulfilment_m\u{2026}",
                "\u{2502}   \u{2514}\u{2500}\u{2500} fix     declare the input shape, then sqb compile -\u{2026}",
                "\u{2514}\u{2500}\u{2500} fct_revenue",
                "    \u{251c}\u{2500}\u{2500} at     models/finance/fct_revenue.sql:42:7",
                "    \u{2514}\u{2500}\u{2500} error  unknown column net_amount_usd",
            ],
        },
    ];

    for test_case in test_cases {
        let rows: Vec<String> = render_tree_group(&test_case.group, &test_case.facts);

        assert_eq!(rows, test_case.expected_rows, "{}", test_case.description);
    }
}
