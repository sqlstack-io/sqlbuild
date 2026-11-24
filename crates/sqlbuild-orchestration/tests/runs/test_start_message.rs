//! A worker's invocation start reads as the command it ran, where and on which version.

use crate::runs::tests::test_types::StartMessageTestCase;
use crate::store::types::Row;
use crate::warehouse_history::_helpers::run_recall::start_message;

#[test]
fn given_an_invocation_start_when_its_event_is_read_then_the_message_names_command_target_and_version()
 {
    let test_cases = [
        StartMessageTestCase {
            description: "every field, with the selection count as SQLite returns it",
            fields: vec![
                ("started_command", serde_json::json!("build")),
                ("started_target", serde_json::json!("uat")),
                ("started_selected", serde_json::json!(93)),
                ("started_version", serde_json::json!("0.259.0")),
            ],
            expected_message: Some("sqb build · target uat · 93 selected · SQLBuild 0.259.0"),
        },
        StartMessageTestCase {
            description: "the selection count as PostgreSQL text, without a target",
            fields: vec![
                ("started_command", serde_json::json!("build")),
                ("started_target", serde_json::Value::Null),
                ("started_selected", serde_json::json!("4")),
                ("started_version", serde_json::json!("0.259.0")),
            ],
            expected_message: Some("sqb build · 4 selected · SQLBuild 0.259.0"),
        },
        StartMessageTestCase {
            description: "another event has no start columns",
            fields: vec![],
            expected_message: None,
        },
    ];
    for test_case in test_cases {
        let row: Row = test_case
            .fields
            .into_iter()
            .map(|(column, value)| (column.to_owned(), value))
            .collect();
        assert_eq!(
            start_message(&row).as_deref(),
            test_case.expected_message,
            "{}",
            test_case.description
        );
    }
}
