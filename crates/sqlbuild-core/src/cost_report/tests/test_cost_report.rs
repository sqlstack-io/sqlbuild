use crate::cost_report::main::format_cost_breakdown::format_cost_breakdown;
use crate::cost_report::main::format_cost_history::format_cost_history;
use crate::cost_report::models::BreakdownOptions;
use crate::cost_report::tests::helpers::{priced, resource, run};
use crate::cost_report::tests::test_types::{BreakdownTestCase, HistoryTestCase};

#[test]
fn given_cost_runs_when_formatting_breakdown_then_table_matches_python_layout() {
    let test_cases = [
        BreakdownTestCase {
            description: "a wide table rounds half to even and scales bytes",
            run: run(
                "2026-09-01T10:05:00+02:00",
                vec![
                    resource("orders", "0.0102875", 5 * 1024 * 1024 * 1024 * 1024 + 1),
                    resource("customers", "1E-7", 1536),
                ],
            ),
            limit: None,
            terminal_width: 140,
            expected_text: "Cost  $0.0206 estimated  |  0.006858 attributed credits  |  default $3.00/credit\nRun  run-1\n\nModel      Warehouse     Busy share  Scanned  Attributed credits  Est. cost\norders     TRANSFORM_WH      12.34s  5.0 TiB            0.003429    $0.0103\ncustomers  TRANSFORM_WH      12.34s  1.5 KiB            0.003429    $0.0000\nTOTAL                        24.69s  1.5 KiB            0.006858    $0.0206\nConfigure cost.usd_per_credit with your Snowflake contract rate, or let the connection's role read SNOWFLAKE.ORGANIZATION_USAGE.RATE_SHEET_DAILY.\nEstimate uses fair-share visible warehouse busy time; it is not Snowflake-billed credits or invoice reconciliation.",
        },
        BreakdownTestCase {
            description: "a narrow limited table notes the hidden resources",
            run: run(
                "2026-09-01T10:05:00+02:00",
                vec![
                    resource("orders_with_a_very_long_model_name_x", "2.5", 0),
                    resource("customers", "0.5", 0),
                ],
            ),
            limit: Some(1),
            terminal_width: 80,
            expected_text: "Cost  $0.0206 estimated  |  0.006858 attributed credits  |  default $3.00/credit\nRun  run-1\n\nModel                             Attributed credits  Est. cost\norders_with_a_very_long_model...            0.003429    $2.5000\nTOTAL                                       0.006858    $0.0206\nShowing 1 of 2 by estimated cost; `sqb cost run-1` for all models (1 more).\nConfigure cost.usd_per_credit with your Snowflake contract rate, or let the connection's role read SNOWFLAKE.ORGANIZATION_USAGE.RATE_SHEET_DAILY.\nEstimate uses fair-share visible warehouse busy time; it is not Snowflake-billed credits or invoice reconciliation.",
        },
        BreakdownTestCase {
            description: "a list price reads as an estimate and names its source",
            run: priced(
                run(
                    "2026-09-01T10:05:00+02:00",
                    vec![resource("orders", "0.5", 0)],
                ),
                "list_price",
            ),
            limit: None,
            terminal_width: 80,
            expected_text: "Cost  $0.0206 estimated  |  0.006858 attributed credits  |  $3.00/credit est. (list price)\nRun  run-1\n\nModel   Attributed credits  Est. cost\norders            0.003429    $0.5000\nTOTAL             0.006858    $0.0206\nConfigure cost.usd_per_credit with your Snowflake contract rate, or let the connection's role read SNOWFLAKE.ORGANIZATION_USAGE.RATE_SHEET_DAILY.\nEstimate uses fair-share visible warehouse busy time; it is not Snowflake-billed credits or invoice reconciliation.",
        },
        BreakdownTestCase {
            description: "the rate sheet's price names its source, with no advice",
            run: priced(
                run(
                    "2026-09-01T10:05:00+02:00",
                    vec![resource("orders", "0.5", 0)],
                ),
                "rate_sheet",
            ),
            limit: None,
            terminal_width: 80,
            expected_text: "Cost  $0.0206 estimated  |  0.006858 attributed credits  |  $3.00/credit (rate sheet)\nRun  run-1\n\nModel   Attributed credits  Est. cost\norders            0.003429    $0.5000\nTOTAL             0.006858    $0.0206\nEstimate uses fair-share visible warehouse busy time; it is not Snowflake-billed credits or invoice reconciliation.",
        },
    ];

    for test_case in test_cases {
        let text: String = format_cost_breakdown(
            &test_case.run,
            BreakdownOptions {
                use_color: false,
                limit: test_case.limit,
                terminal_width: test_case.terminal_width,
                show_run_id: true,
            },
        );
        assert_eq!(text, test_case.expected_text, "{}", test_case.description);
    }
}

#[test]
fn given_offset_timestamps_when_formatting_history_then_completion_is_shown_in_utc() {
    let test_cases = [
        HistoryTestCase {
            description: "a negative offset crosses midnight into the next day",
            completed_at: "2026-12-31T23:30:00.5-05:00",
            expected_text: "SQLBuild cost history\nCompleted (UTC)      Run    Build    Cost      Attributed credits  Est. USD\n2027-01-01 04:30:00  run-1  success  complete            0.006858   $0.0206\nShowing 1 of 3 matching runs. Run `sqb cost <run_id>` for a full breakdown.",
        },
        HistoryTestCase {
            description: "a positive offset on a leap day moves back a day",
            completed_at: "2028-03-01T01:00:00+02:00",
            expected_text: "SQLBuild cost history\nCompleted (UTC)      Run    Build    Cost      Attributed credits  Est. USD\n2028-02-29 23:00:00  run-1  success  complete            0.006858   $0.0206\nShowing 1 of 3 matching runs. Run `sqb cost <run_id>` for a full breakdown.",
        },
    ];

    for test_case in test_cases {
        let text: String =
            format_cost_history(&[run(test_case.completed_at, Vec::new())], 3, false);
        assert_eq!(text, test_case.expected_text, "{}", test_case.description);
    }
}
