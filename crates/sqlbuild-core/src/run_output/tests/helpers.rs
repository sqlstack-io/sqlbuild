//! Inputs shared by the run output tests.

use crate::run_output::models::{
    ExplainRow, FailureEntry, FailureField, FailureRow, HeaderCause, ProgressFooter, RowTone,
    RunCounts, RunHeader,
};

/// A long test name that a narrow terminal cuts in the middle.
pub(super) const LONG_NAME: &str =
    "fct_orders__totals_match_line_items_after_refunds_and_partial_shipments_are_applied";

/// A footer of 461 tests with `running` executing, `held` waiting to print and the oldest named.
pub(super) fn footer(running: usize, held: usize, oldest: Option<(&str, f64)>) -> ProgressFooter {
    ProgressFooter {
        verb: "Testing".to_owned(),
        noun: "tests".to_owned(),
        counts: RunCounts {
            pass: 120,
            fail: 3,
            timeout: 0,
            not_run: 7,
            total: 461,
        },
        running,
        held,
        oldest: oldest.map(|(name, seconds)| (name.to_owned(), seconds)),
        elapsed: 42.5,
    }
}

/// A failed test with an error, a labelled field, a path, a row diff and a code frame.
pub(super) fn failure_entry(name: &str) -> FailureEntry {
    FailureEntry {
        name: name.to_owned(),
        meta: "test, local, 0.42s".to_owned(),
        fields: vec![
            FailureField::Error {
                code: Some("T2001".to_owned()),
                message: "expected rows differ from the model's rows: 1 row only expected, 1 row only actual, compared on order_id and status".to_owned(),
                help: Some("run with --verbose to see every differing row".to_owned()),
            },
            FailureField::Labelled {
                label: "model".to_owned(),
                text: "fct_orders".to_owned(),
            },
            FailureField::Verbatim("tests/sql/fct_orders/totals.sql".to_owned()),
            FailureField::Rows(vec![
                FailureRow {
                    tone: RowTone::Header,
                    text: "order_id  status".to_owned(),
                    note: String::new(),
                },
                FailureRow {
                    tone: RowTone::Expected,
                    text: "- 1001      shipped".to_owned(),
                    note: "expected".to_owned(),
                },
                FailureRow {
                    tone: RowTone::Actual,
                    text: "+ 1001      refunded".to_owned(),
                    note: "actual".to_owned(),
                },
            ]),
            FailureField::Frame {
                line: 12,
                column: 10,
                width: 6,
                snippet: "select amount / 0 as ratio".to_owned(),
            },
        ],
    }
}

/// `count` failed tests named `test_01`, `test_02`, … with the same reason.
pub(super) fn failed_list(count: usize) -> Vec<(String, String)> {
    (1..=count)
        .map(|index| (format!("test_{index:02}"), "Binder Error".to_owned()))
        .collect()
}

/// `count` causes `fn:FUNCTION_<n>`, keeping 6, 5, 4, 3, 2 then 1 tests off DuckDB.
pub(super) fn causes(count: usize) -> Vec<HeaderCause> {
    (0..count)
        .map(|index| HeaderCause {
            label: format!("fn:FUNCTION_{index}"),
            count: 6 - index.min(5),
            detail: "SQ0002 \u{b7} 1 model".to_owned(),
        })
        .collect()
}

/// A header of 40 tests: how many run locally, on the warehouse and not at all, and the causes.
pub(super) fn header(
    causes: Vec<HeaderCause>,
    (local, warehouse, not_run): (usize, usize, usize),
    local_only: bool,
) -> RunHeader {
    RunHeader {
        title: "Tests".to_owned(),
        nouns: ("test".to_owned(), "tests".to_owned()),
        selected: 40,
        local,
        warehouse,
        not_run,
        local_only,
        causes,
        explain: Vec::new(),
    }
}

/// `--explain`'s rows for the first two causes: one in a model, one in a macro.
pub(super) fn explain_rows() -> Vec<ExplainRow> {
    vec![
        ExplainRow {
            count: 6,
            reason: "fn:FUNCTION_0".to_owned(),
            place: "sql/models/staging/stg_orders.sql:14".to_owned(),
        },
        ExplainRow {
            count: 5,
            reason: "fn:FUNCTION_1".to_owned(),
            place: "macro cents_to_dollars".to_owned(),
        },
    ]
}
