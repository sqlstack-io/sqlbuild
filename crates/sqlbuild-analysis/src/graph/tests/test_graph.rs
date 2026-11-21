use crate::graph::tests::helpers::{failure, keys, resolved};
use crate::graph::tests::test_types::{
    CloseMatchTestCase, SelectorErrorTestCase, SelectorTestCase,
};
use sqlbuild_core::text::main::close_matches::close_matches;

#[test]
fn given_selectors_when_resolving_then_keys_match_python() {
    let test_cases = [
        SelectorTestCase {
            description: "nothing selected",
            select: &[],
            exclude: &[],
            expected_keys: &[
                ("model", "customer_orders"),
                ("model", "customers"),
                ("model", "order_items"),
                ("model", "orders"),
                ("seed", "countries"),
                ("source", "raw_orders"),
                ("udf", "tax"),
            ],
        },
        SelectorTestCase {
            description: "orders",
            select: &["orders"],
            exclude: &[],
            expected_keys: &[("model", "orders"), ("udf", "tax")],
        },
        SelectorTestCase {
            description: "+orders",
            select: &["+orders"],
            exclude: &[],
            expected_keys: &[
                ("model", "orders"),
                ("source", "raw_orders"),
                ("udf", "tax"),
            ],
        },
        SelectorTestCase {
            description: "orders+",
            select: &["orders+"],
            exclude: &[],
            expected_keys: &[
                ("model", "customer_orders"),
                ("model", "order_items"),
                ("model", "orders"),
                ("udf", "tax"),
            ],
        },
        SelectorTestCase {
            description: "tag:daily minus order_items",
            select: &["tag:daily"],
            exclude: &["order_items"],
            expected_keys: &[("model", "orders"), ("udf", "tax")],
        },
        SelectorTestCase {
            description: "models/mart",
            select: &["models/mart"],
            exclude: &[],
            expected_keys: &[
                ("model", "order_items"),
                ("model", "orders"),
                ("udf", "tax"),
            ],
        },
        SelectorTestCase {
            description: "path:models/staging",
            select: &["path:models/staging"],
            exclude: &[],
            expected_keys: &[("model", "customers")],
        },
        SelectorTestCase {
            description: "order*",
            select: &["order*"],
            exclude: &[],
            expected_keys: &[
                ("model", "order_items"),
                ("model", "orders"),
                ("udf", "tax"),
            ],
        },
        SelectorTestCase {
            description: "seed:countries+",
            select: &["seed:countries+"],
            exclude: &[],
            expected_keys: &[
                ("model", "order_items"),
                ("seed", "countries"),
                ("udf", "tax"),
            ],
        },
        SelectorTestCase {
            description: "customers~customer_orders",
            select: &["customers~customer_orders"],
            exclude: &[],
            expected_keys: &[
                ("model", "customer_orders"),
                ("model", "customers"),
                ("udf", "tax"),
            ],
        },
        SelectorTestCase {
            description: "+orders~order_items+",
            select: &["+orders~order_items+"],
            exclude: &[],
            expected_keys: &[
                ("model", "order_items"),
                ("model", "orders"),
                ("source", "raw_orders"),
                ("udf", "tax"),
            ],
        },
        SelectorTestCase {
            description: "tag:daily,models/mart/items",
            select: &["tag:daily,models/mart/items"],
            exclude: &[],
            expected_keys: &[("model", "order_items"), ("udf", "tax")],
        },
        SelectorTestCase {
            description: "orders customers",
            select: &["orders customers"],
            exclude: &[],
            expected_keys: &[("model", "customers"), ("model", "orders"), ("udf", "tax")],
        },
        SelectorTestCase {
            description: " minus orders",
            select: &[],
            exclude: &["orders"],
            expected_keys: &[
                ("model", "customer_orders"),
                ("model", "customers"),
                ("model", "order_items"),
                ("seed", "countries"),
                ("source", "raw_orders"),
                ("udf", "tax"),
            ],
        },
        SelectorTestCase {
            description: "source:raw_*",
            select: &["source:raw_*"],
            exclude: &[],
            expected_keys: &[("source", "raw_orders")],
        },
        SelectorTestCase {
            description: "[!o]*",
            select: &["[!o]*"],
            exclude: &[],
            expected_keys: &[
                ("model", "customer_orders"),
                ("model", "customers"),
                ("seed", "countries"),
                ("source", "raw_orders"),
                ("udf", "tax"),
            ],
        },
    ];

    for test_case in test_cases {
        assert_eq!(
            resolved(test_case.select, test_case.exclude),
            Ok(keys(test_case.expected_keys)),
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_invalid_selectors_when_resolving_then_errors_match_python() {
    let test_cases = [
        SelectorErrorTestCase {
            description: "models",
            select: &["models"],
            exclude: &[],
            expected_error: ("S007", "unknown selector name 'models'", None),
        },
        SelectorErrorTestCase {
            description: "ordrs",
            select: &["ordrs"],
            exclude: &[],
            expected_error: (
                "S007",
                "unknown selector name 'ordrs'",
                Some("did you mean 'orders'?"),
            ),
        },
        SelectorErrorTestCase {
            description: "ord*x",
            select: &["ord*x"],
            exclude: &[],
            expected_error: ("S007", "unknown selector pattern 'ord*x'", None),
        },
        SelectorErrorTestCase {
            description: "a+b",
            select: &["a+b"],
            exclude: &[],
            expected_error: (
                "S002",
                "selector 'a+b' contains '+' in an unsupported position",
                None,
            ),
        },
        SelectorErrorTestCase {
            description: "+",
            select: &["+"],
            exclude: &[],
            expected_error: (
                "S004",
                "selector '+' has no name after removing '+' markers",
                None,
            ),
        },
        SelectorErrorTestCase {
            description: "a,,b",
            select: &["a,,b"],
            exclude: &[],
            expected_error: ("S007", "unknown selector name 'a'", None),
        },
        SelectorErrorTestCase {
            description: "x~",
            select: &["x~"],
            exclude: &[],
            expected_error: (
                "S003",
                "path selector 'x~' requires names on both sides of '~'",
                None,
            ),
        },
        SelectorErrorTestCase {
            description: "foo:bar",
            select: &["foo:bar"],
            exclude: &[],
            expected_error: ("S005", "unknown selector type 'foo' in 'foo:bar'", None),
        },
        SelectorErrorTestCase {
            description: "seed:",
            select: &["seed:"],
            exclude: &[],
            expected_error: ("S006", "selector 'seed:' has empty value after ':'", None),
        },
        SelectorErrorTestCase {
            description: "tag:none",
            select: &["tag:none"],
            exclude: &[],
            expected_error: ("S008", "no models found with tag 'none'", None),
        },
        SelectorErrorTestCase {
            description: "staging/crm.sql",
            select: &["staging/crm.sql"],
            exclude: &[],
            expected_error: (
                "S012",
                "path selectors require an explicit root: use 'models/' or 'python/'",
                None,
            ),
        },
        SelectorErrorTestCase {
            description: "mart",
            select: &["mart"],
            exclude: &[],
            expected_error: ("S007", "unknown selector name 'mart'", None),
        },
        SelectorErrorTestCase {
            description: "test:abc",
            select: &["test:abc"],
            exclude: &[],
            expected_error: (
                "S013",
                "selector 'test:abc' selects a unit test; only `sqb test` and `sqb build` accept unit-test selectors",
                None,
            ),
        },
        SelectorErrorTestCase {
            description: "asset:abc",
            select: &["asset:abc"],
            exclude: &[],
            expected_error: (
                "S010",
                "selector type 'asset' does not map to a resource type yet",
                None,
            ),
        },
        SelectorErrorTestCase {
            description: "order_items~orders",
            select: &["order_items~orders"],
            exclude: &[],
            expected_error: (
                "S000",
                "'model:orders' is not downstream of 'model:order_items'",
                None,
            ),
        },
        SelectorErrorTestCase {
            description: "customer",
            select: &["customer"],
            exclude: &[],
            expected_error: (
                "S007",
                "unknown selector name 'customer'",
                Some("did you mean 'customers'?"),
            ),
        },
    ];

    for test_case in test_cases {
        assert_eq!(
            resolved(test_case.select, test_case.exclude),
            Err(failure(test_case.expected_error)),
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_unknown_name_when_suggesting_then_matches_get_close_matches() {
    let test_cases = [
        CloseMatchTestCase {
            description: "close matches for ordrs",
            word: "ordrs",
            candidates: &[
                "customer_orders",
                "customers",
                "order_items",
                "orders",
                "raw_orders",
            ],
            cutoff: 0.8,
            expected_matches: &["orders"],
        },
        CloseMatchTestCase {
            description: "close matches for custmer",
            word: "custmer",
            candidates: &["costumer", "custom", "customer", "customers"],
            cutoff: 0.6,
            expected_matches: &["customer", "customers", "costumer"],
        },
    ];

    for test_case in test_cases {
        assert_eq!(
            close_matches(test_case.word, test_case.candidates, 3, test_case.cutoff),
            test_case.expected_matches,
            "{}",
            test_case.description
        );
    }
}
