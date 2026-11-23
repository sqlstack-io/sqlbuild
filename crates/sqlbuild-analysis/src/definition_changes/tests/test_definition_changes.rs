use serde_json::json;

use crate::definition_changes::constants::DEFINITION_SNAPSHOT_FORMAT;
use crate::definition_changes::main::changes_summary::changes_summary;
use crate::definition_changes::main::compare_definitions::compare_definitions;
use crate::definition_changes::main::definition_snapshot_json::definition_snapshot_json;
use crate::definition_changes::main::parse_definition_snapshot::parse_definition_snapshot;
use crate::definition_changes::main::selection_json::selection_json;
use crate::definition_changes::main::selection_reasons::selection_reasons;
use crate::definition_changes::main::selection_report::selection_report;
use crate::definition_changes::main::sql_definition_digest::sql_definition_digest;
use crate::definition_changes::main::sql_macro_definition_digest::sql_macro_definition_digest;
use crate::definition_changes::models::{DefinitionChanges, SelectedItem};
use crate::definition_changes::tests::helpers::{
    base_orders, current_orders, item, keys, mocking, node, renamed, snapshot, with_node,
};
use crate::definition_changes::tests::test_types::{
    CompareTestCase, SelectionJsonTestCase, SelectionReasonTestCase, SelectionReportTestCase,
    SnapshotJsonTestCase, SqlDigestTestCase,
};

#[test]
fn given_two_snapshots_when_comparing_then_reports_changed_renamed_and_deleted_nodes() {
    let test_cases = [
        CompareTestCase {
            description: "an edit, a pure rename and a deletion",
            base: base_orders(),
            current: current_orders(),
            expected_changes: &[("model:stg_orders", &["sql"], false, None)],
            expected_renamed: &[("model:old_customers", "model:customers")],
            expected_deleted: &["seed countries"],
            expected_summary: "1 model changed; 1 model renamed; deleted: seed countries",
        },
        CompareTestCase {
            description: "a rename whose definition also changed",
            base: snapshot(vec![node("model", "old_customers", &[("sql", "s3")], &[])]),
            current: snapshot(vec![renamed(
                node(
                    "model",
                    "customers",
                    &[("sql", "s4"), ("config", "c4")],
                    &[],
                ),
                &["old_customers"],
            )]),
            expected_changes: &[(
                "model:customers",
                &["sql", "config"],
                false,
                Some("old_customers"),
            )],
            expected_renamed: &[("model:old_customers", "model:customers")],
            expected_deleted: &[],
            expected_summary: "1 model changed; 1 model renamed",
        },
        CompareTestCase {
            description: "added nodes count as changed and kinds are counted",
            base: base_orders(),
            current: with_node(
                with_node(
                    current_orders(),
                    node("model", "returns", &[("sql", "s9")], &[]),
                ),
                node("macro", "to_timestamp", &[("definition", "m3")], &[]),
            ),
            expected_changes: &[
                ("macro:to_timestamp", &["definition"], false, None),
                ("model:returns", &[], true, None),
                ("model:stg_orders", &["sql"], false, None),
            ],
            expected_renamed: &[("model:old_customers", "model:customers")],
            expected_deleted: &["seed countries"],
            expected_summary: "1 model, 1 macro changed; 1 model added; 1 model renamed; \
                               deleted: seed countries",
        },
        CompareTestCase {
            description: "identical snapshots",
            base: base_orders(),
            current: base_orders(),
            expected_changes: &[],
            expected_renamed: &[],
            expected_deleted: &[],
            expected_summary: "no definition changes",
        },
    ];
    for test_case in test_cases {
        let changes: DefinitionChanges = compare_definitions(&test_case.base, &test_case.current);

        let observed: Vec<(String, Vec<String>, bool, Option<String>)> = changes
            .changed
            .iter()
            .map(|(key, change)| {
                (
                    key.clone(),
                    change.aspects.clone(),
                    change.added,
                    change.renamed_from.clone(),
                )
            })
            .collect();
        let expected: Vec<(String, Vec<String>, bool, Option<String>)> = test_case
            .expected_changes
            .iter()
            .map(|(key, aspects, added, renamed_from)| {
                (
                    (*key).to_owned(),
                    keys(aspects),
                    *added,
                    renamed_from.map(str::to_owned),
                )
            })
            .collect();
        let deleted: Vec<String> = changes
            .deleted
            .iter()
            .map(|node| format!("{} {}", node.kind, node.name))
            .collect();
        let renamed: Vec<(String, String)> = test_case
            .expected_renamed
            .iter()
            .map(|(old, new)| ((*old).to_owned(), (*new).to_owned()))
            .collect();
        assert_eq!(observed, expected, "{}", test_case.description);
        assert_eq!(changes.renamed, renamed, "{}", test_case.description);
        assert_eq!(
            deleted,
            keys(test_case.expected_deleted),
            "{}",
            test_case.description
        );
        assert_eq!(
            changes_summary(&changes),
            test_case.expected_summary,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_executed_item_when_explaining_selection_then_lists_each_changed_dependency() {
    let test_cases = [
        SelectionReasonTestCase {
            description: "an executed model whose SQL changed",
            current: current_orders(),
            item: item(
                &["test_file:tests/orders.sql"],
                &["model:stg_orders", "model:orders"],
                &[],
                &[],
            ),
            expected_reasons: &["executes stg_orders (model SQL changed)"],
        },
        SelectionReasonTestCase {
            description: "a mocked model is not executed, so its change selects nothing",
            current: current_orders(),
            item: item(&["test_file:tests/orders.sql"], &["model:orders"], &[], &[]),
            expected_reasons: &[],
        },
        SelectionReasonTestCase {
            description: "a mocked model's SQL change does not select the test",
            current: current_orders(),
            item: mocking(
                item(&["test_file:tests/orders.sql"], &["model:orders"], &[], &[]),
                &["model:stg_orders"],
            ),
            expected_reasons: &[],
        },
        SelectionReasonTestCase {
            description: "a mocked model's declared columns shape the mock",
            current: with_node(
                base_orders(),
                node(
                    "model",
                    "stg_orders",
                    &[("sql", "s1"), ("config", "c1"), ("columns", "k2")],
                    &["macro:to_utc"],
                ),
            ),
            item: mocking(
                item(&["test_file:tests/orders.sql"], &["model:orders"], &[], &[]),
                &["model:stg_orders"],
            ),
            expected_reasons: &["mocks stg_orders (model columns changed)"],
        },
        SelectionReasonTestCase {
            description: "a macro changed through another macro the model calls",
            current: with_node(
                base_orders(),
                node("macro", "to_timestamp", &[("definition", "m3")], &[]),
            ),
            item: item(&[], &["model:stg_orders"], &[], &[]),
            expected_reasons: &["executes macro to_timestamp (changed)"],
        },
        SelectionReasonTestCase {
            description: "a mocked macro hides its own and its callees' changes",
            current: with_node(
                base_orders(),
                node("macro", "to_timestamp", &[("definition", "m3")], &[]),
            ),
            item: item(&[], &["model:stg_orders"], &[], &["macro:to_utc"]),
            expected_reasons: &[],
        },
        SelectionReasonTestCase {
            description: "an enum used by an executed model",
            current: with_node(
                base_orders(),
                node("enum", "order_status", &[("definition", "e2")], &[]),
            ),
            item: item(&[], &["model:orders"], &[], &[]),
            expected_reasons: &["uses enum order_status (changed)"],
        },
        SelectionReasonTestCase {
            description: "the test's own file and an added model",
            current: with_node(
                with_node(
                    base_orders(),
                    node("test_file", "tests/orders.sql", &[("file", "t2")], &[]),
                ),
                node("model", "returns", &[("sql", "s9")], &[]),
            ),
            item: item(
                &["test_file:tests/orders.sql"],
                &["model:returns"],
                &[],
                &[],
            ),
            expected_reasons: &["test file changed", "executes returns (added)"],
        },
        SelectionReasonTestCase {
            description: "a renamed model whose config changed",
            current: snapshot(vec![renamed(
                node(
                    "model",
                    "customers",
                    &[("sql", "s3"), ("config", "c9")],
                    &[],
                ),
                &["old_customers"],
            )]),
            item: item(&[], &["model:customers"], &[], &[]),
            expected_reasons: &[
                "executes customers (renamed from old_customers, model config changed)",
            ],
        },
        SelectionReasonTestCase {
            description: "a changed seed the test really reads",
            current: with_node(
                base_orders(),
                node("seed", "countries", &[("definition", "d2")], &[]),
            ),
            item: item(&[], &["model:orders", "seed:countries"], &[], &[]),
            expected_reasons: &["reads seed countries (changed)"],
        },
    ];
    for test_case in test_cases {
        let changes: DefinitionChanges = compare_definitions(&base_orders(), &test_case.current);

        let reasons: Vec<String> = selection_reasons(&test_case.item, &test_case.current, &changes);

        assert_eq!(
            reasons,
            keys(test_case.expected_reasons),
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_selected_items_when_reporting_then_counts_and_explains_only_on_request() {
    let selected = || {
        vec![SelectedItem {
            label: "orders_rollup".to_owned(),
            reasons: keys(&["executes stg_orders (model SQL changed)"]),
        }]
    };
    let test_cases = [
        SelectionReportTestCase {
            description: "explain lists each item's reasons",
            summary: "1 model changed",
            nouns: ("test", "tests"),
            total: 9,
            selected: selected(),
            explain: true,
            expected_text: "Changed since origin/main: 1 model changed\nSelected 1 of 9 tests\n  \
                            orders_rollup\n    executes stg_orders (model SQL changed)\n",
        },
        SelectionReportTestCase {
            description: "without explain only counts are shown",
            summary: "1 model changed",
            nouns: ("test", "tests"),
            total: 9,
            selected: selected(),
            explain: false,
            expected_text: "Changed since origin/main: 1 model changed\nSelected 1 of 9 tests\n",
        },
        SelectionReportTestCase {
            description: "nothing affected says so",
            summary: "no definition changes",
            nouns: ("scenario", "scenarios"),
            total: 3,
            selected: Vec::new(),
            explain: true,
            expected_text: "Changed since origin/main: no definition changes\nNo scenarios are \
                            affected by the changes since origin/main\n",
        },
    ];
    for test_case in test_cases {
        let text: String = selection_report(
            ("origin/main", test_case.summary),
            test_case.nouns,
            (test_case.total, &test_case.selected),
            test_case.explain,
        );

        assert_eq!(text, test_case.expected_text, "{}", test_case.description);
    }
}

#[test]
fn given_stored_snapshot_when_reading_then_accepts_only_the_same_format() {
    let test_cases = [
        SnapshotJsonTestCase {
            description: "a stored snapshot reads back",
            stored: |text| text,
            expected_snapshot: Some(base_orders()),
        },
        SnapshotJsonTestCase {
            description: "another format version is recomputed",
            stored: |text| {
                text.replacen(
                    &format!("\"format\":{DEFINITION_SNAPSHOT_FORMAT}"),
                    "\"format\":0",
                    1,
                )
            },
            expected_snapshot: None,
        },
        SnapshotJsonTestCase {
            description: "unreadable text is recomputed",
            stored: |_| "not json".to_owned(),
            expected_snapshot: None,
        },
    ];
    for test_case in test_cases {
        let text: String = (test_case.stored)(definition_snapshot_json(&base_orders()));

        assert_eq!(
            parse_definition_snapshot(&text),
            test_case.expected_snapshot,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_sql_when_fingerprinting_definition_then_ignores_layout_and_comments_only() {
    let original: String = sql_definition_digest("SELECT order_id FROM orders");
    let test_cases = [
        SqlDigestTestCase {
            description: "layout, comments and keyword case are ignored",
            sql: "select\n  order_id -- the key\nfrom orders",
            expected_same_as_original: true,
        },
        SqlDigestTestCase {
            description: "a different column is a change",
            sql: "SELECT customer_id FROM orders",
            expected_same_as_original: false,
        },
    ];
    for test_case in test_cases {
        let digest: String = sql_definition_digest(test_case.sql);

        assert_eq!(
            digest == original,
            test_case.expected_same_as_original,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_changes_and_selection_when_writing_json_then_lists_nodes_and_reasons() {
    let test_cases = [SelectionJsonTestCase {
        description: "changed, renamed and deleted nodes with each selected item's reasons",
        selected: vec![SelectedItem {
            label: "orders_rollup".to_owned(),
            reasons: keys(&["executes stg_orders (model SQL changed)"]),
        }],
        expected_json: json!({
            "ref": "origin/main",
            "merge_base": "abc123",
            "changed": [{
                "kind": "model",
                "name": "stg_orders",
                "added": false,
                "aspects": ["sql"],
                "renamed_from": null,
            }],
            "renamed": [{ "kind": "model", "from": "old_customers", "to": "customers" }],
            "deleted": [{ "kind": "seed", "name": "countries" }],
            "candidate_count": 9,
            "selected": [{
                "name": "orders_rollup",
                "reasons": ["executes stg_orders (model SQL changed)"],
            }],
        }),
    }];
    for test_case in test_cases {
        let changes: DefinitionChanges = compare_definitions(&base_orders(), &current_orders());

        let document: serde_json::Value = selection_json(
            ("origin/main", "abc123"),
            &changes,
            (9, &test_case.selected),
        );

        assert_eq!(
            document, test_case.expected_json,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_a_sql_macro_when_fingerprinting_definition_then_ignores_layout_comments_and_description() {
    let path: &str = "sql/macros/sql/net.sql";
    let original: String = sql_macro_definition_digest(
        "net",
        path,
        "MACRO (\n  description 'Net.',\n  arguments (cents INTEGER),\n  returns DOUBLE,\n);\n\n\
         @cents / 100 - 1\n",
    );
    let test_cases = [
        SqlDigestTestCase {
            description: "header layout, a trailing comma, comments and the description are ignored",
            sql: "MACRO (description 'Net after the fee.', arguments (cents integer), \
                  returns double);\n\n-- one dollar fee\n@cents/100   -   1\n",
            expected_same_as_original: true,
        },
        SqlDigestTestCase {
            description: "a body change is a change",
            sql: "MACRO (description 'Net.', arguments (cents INTEGER), returns DOUBLE);\n\n\
                  @cents / 100 - 2\n",
            expected_same_as_original: false,
        },
        SqlDigestTestCase {
            description: "a parameter type change is a change",
            sql: "MACRO (description 'Net.', arguments (cents BIGINT), returns DOUBLE);\n\n\
                  @cents / 100 - 1\n",
            expected_same_as_original: false,
        },
        SqlDigestTestCase {
            description: "a result type change is a change",
            sql: "MACRO (description 'Net.', arguments (cents INTEGER), returns DECIMAL(18, 2));\n\n\
                  @cents / 100 - 1\n",
            expected_same_as_original: false,
        },
    ];
    for test_case in test_cases {
        let digest: String = sql_macro_definition_digest("net", path, test_case.sql);

        assert_eq!(
            digest == original,
            test_case.expected_same_as_original,
            "{}",
            test_case.description
        );
    }
}
