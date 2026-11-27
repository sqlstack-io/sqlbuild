use crate::cli_text::models::CliStyle;
use crate::nested_progress::main::render_item_group::render_item_group;
use crate::nested_progress::main::render_nested_item::render_nested_item;
use crate::nested_progress::models::NestedLayout;
use crate::nested_progress::tests::helpers::{errored, item, layout};
use crate::nested_progress::tests::test_types::{GroupTestCase, NestedItemTestCase};

const PLAIN: CliStyle = CliStyle { use_color: false };
const LONG_NAME: &str =
    "fct_orders__totals_match_line_items_after_refunds_and_partial_shipments_are_applied";

#[test]
fn given_completed_item_when_rendering_then_draws_one_row_and_children_only_on_failure() {
    let test_cases = [
        NestedItemTestCase {
            description: "a pass at 80 columns is one row without detail",
            item: item("orders_assert", "PASS"),
            group: "",
            layout: layout(Some(80), false),
            expected_text: "    7/461  test      orders_assert                   PASS   local        0.42s\n",
        },
        NestedItemTestCase {
            description: "a pass at 160 columns adds the dim detail",
            item: item("orders_assert", "PASS"),
            group: "",
            layout: layout(Some(160), false),
            expected_text: "    7/461  test      orders_assert                   PASS   local        0.42s  1 real model\n",
        },
        NestedItemTestCase {
            description: "--verbose shows a passing item's children",
            item: item("orders_assert", "PASS"),
            group: "",
            layout: layout(Some(80), true),
            expected_text: "    7/461  test      orders_assert                   PASS   local        0.42s\n             └── expected  fct_orders                PASS   1 row differs\n",
        },
        NestedItemTestCase {
            description: "a row under its group drops the group prefix",
            item: item("fct_orders__totals_match", "FAIL"),
            group: "fct_orders",
            layout: layout(Some(80), false),
            expected_text: "    7/461  test      totals_match                    FAIL   local        0.42s\n             └── expected  fct_orders                FAIL   1 row differs\n",
        },
        NestedItemTestCase {
            description: "a long name is cut in the middle at 80 columns",
            item: item(LONG_NAME, "PASS"),
            group: "",
            layout: layout(Some(80), false),
            expected_text: "    7/461  test      fct_orders__t...ts_are_applied  PASS   local        0.42s\n",
        },
        NestedItemTestCase {
            description: "without a terminal width a long name is shown in full, with its detail",
            item: item(LONG_NAME, "PASS"),
            group: "",
            layout: NestedLayout {
                name_width: LONG_NAME.len(),
                ..layout(None, false)
            },
            expected_text: "    7/461  test      fct_orders__totals_match_line_items_after_refunds_and_partial_shipments_are_applied  PASS   local        0.42s  1 real model\n",
        },
        NestedItemTestCase {
            description: "a wide terminal cuts nothing it has room for",
            item: item(LONG_NAME, "PASS"),
            group: "",
            layout: NestedLayout {
                name_width: LONG_NAME.len(),
                ..layout(Some(200), false)
            },
            expected_text: "    7/461  test      fct_orders__totals_match_line_items_after_refunds_and_partial_shipments_are_applied  PASS   local        0.42s  1 real model\n",
        },
        NestedItemTestCase {
            description: "without a terminal width an error is neither wrapped nor cut",
            item: errored("orders_assert"),
            group: "",
            layout: layout(None, false),
            expected_text: "    7/461  test      orders_assert                   FAIL   local        0.42s  1 real model\n             └── error     [T2001] Binder Error: Referenced column \"amount\" not found in FROM clause\n                           = help: check the model's columns\n",
        },
        NestedItemTestCase {
            description: "an error is the last child, wrapped to the terminal",
            item: errored("orders_assert"),
            group: "",
            layout: layout(Some(80), false),
            expected_text: "    7/461  test      orders_assert                   FAIL   local        0.42s\n             └── error     [T2001] Binder Error: Referenced column \"amount\" not\n                           found in FROM clause...\n",
        },
    ];

    for test_case in test_cases {
        let text = render_nested_item(&test_case.item, test_case.group, &test_case.layout, PLAIN);
        assert_eq!(text, test_case.expected_text, "{}", test_case.description);
    }
}

#[test]
fn given_group_when_heading_then_counts_its_tests() {
    let test_cases = [
        GroupTestCase {
            description: "one test is singular",
            count: 1,
            expected_text: "\n  fct_orders  (1 test)\n",
        },
        GroupTestCase {
            description: "several tests are plural",
            count: 3,
            expected_text: "\n  fct_orders  (3 tests)\n",
        },
    ];

    for test_case in test_cases {
        assert_eq!(
            render_item_group("fct_orders", test_case.count, ("test", "tests"), PLAIN),
            test_case.expected_text,
            "{}",
            test_case.description
        );
    }
}
