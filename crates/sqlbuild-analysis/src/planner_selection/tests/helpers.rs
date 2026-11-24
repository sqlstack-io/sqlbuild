use std::collections::BTreeSet;

use crate::graph::errors::SelectorError;
use crate::planner_selection::models::{
    SelectableModel, UnitTestSelectorNames, UnitTestSelectorSplit,
};

pub(super) fn texts(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
}

pub(super) fn name_set(values: &[&str]) -> BTreeSet<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
}

/// Unit tests `test_orders` and `test_customers`; resources `orders` and `customers`.
pub(super) fn selector_names() -> UnitTestSelectorNames {
    UnitTestSelectorNames {
        tests: name_set(&["test_customers", "test_orders"]),
        resources: name_set(&["customers", "orders", "step[a]", "step[b]"]),
    }
}

pub(super) fn split(
    select: &[&str],
    tests: &[&str],
    models_selected: bool,
) -> UnitTestSelectorSplit {
    UnitTestSelectorSplit {
        select: texts(select),
        exclude: Vec::new(),
        test_names: name_set(tests),
        excluded_test_names: BTreeSet::new(),
        models_selected,
    }
}

pub(super) fn failure(code: &'static str, message: &str, help: Option<&str>) -> SelectorError {
    SelectorError {
        code,
        message: message.to_owned(),
        help: help.map(str::to_owned),
    }
}

pub(super) fn models() -> Vec<SelectableModel> {
    let model = |name: &str, tags: &[&str], folder: &str| SelectableModel {
        name: name.to_owned(),
        tags: texts(tags),
        folder: folder.to_owned(),
    };
    vec![
        model("orders", &["daily"], "sql/models/staging"),
        model("customers", &[], "sql/models/staging/sub"),
        model("report", &["daily"], "sql/models/marts"),
    ]
}

/// Unit tests `order_status`, `order_totals_check` and `customer_status` beside four resources.
pub(super) fn order_selector_names() -> UnitTestSelectorNames {
    UnitTestSelectorNames {
        tests: name_set(&["customer_status", "order_status", "order_totals_check"]),
        resources: name_set(&["customers", "order_totals", "orders", "raw_orders"]),
    }
}

/// A split with resource selectors, selected and excluded unit tests, and whether models run.
pub(super) fn full_split(
    (select, exclude): (&[&str], &[&str]),
    (tests, excluded_tests): (&[&str], &[&str]),
    models_selected: bool,
) -> UnitTestSelectorSplit {
    UnitTestSelectorSplit {
        select: texts(select),
        exclude: texts(exclude),
        test_names: name_set(tests),
        excluded_test_names: name_set(excluded_tests),
        models_selected,
    }
}
