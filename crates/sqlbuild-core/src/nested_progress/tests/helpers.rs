use crate::build_progress::models::ErrorText;
use crate::nested_progress::models::{NestedChildRow, NestedItem, NestedLayout};

/// Rows of 461 tests at `columns`, with the longest name 30 characters.
pub(super) fn layout(columns: usize, verbose: bool) -> NestedLayout {
    NestedLayout {
        label: "test".to_owned(),
        name_width: 30,
        columns,
        total: 461,
        verbose,
    }
}

/// Item 7 of the run, local, with `status` and one expected-rows child.
pub(super) fn item(name: &str, status: &str) -> NestedItem {
    NestedItem {
        item_name: name.to_owned(),
        position: 7,
        status_text: status.to_owned(),
        run_on: "local".to_owned(),
        seconds: Some(0.42),
        detail: "1 real model".to_owned(),
        error: ErrorText::default(),
        child_rows: vec![NestedChildRow {
            label: "expected".to_owned(),
            name: "fct_orders".to_owned(),
            status_text: status.to_owned(),
            detail: "1 row differs".to_owned(),
        }],
    }
}

/// A failed item whose error has a code and help.
pub(super) fn errored(name: &str) -> NestedItem {
    NestedItem {
        status_text: "FAIL".to_owned(),
        error: ErrorText {
            code: Some("T2001".to_owned()),
            message: Some(
                "Binder Error: Referenced column \"amount\" not found in FROM clause".to_owned(),
            ),
            help: Some("check the model's columns".to_owned()),
        },
        child_rows: Vec::new(),
        ..item(name, "FAIL")
    }
}
