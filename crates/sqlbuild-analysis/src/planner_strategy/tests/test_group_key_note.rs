use crate::planner_strategy::main::group_key_note::group_key_note;
use crate::planner_strategy::models::{MaterializationType, WarningSeverity};
use crate::planner_strategy::tests::test_types::GroupKeyNoteTestCase;

#[test]
fn given_delete_insert_keys_when_planning_then_only_unique_key_matching_gets_the_note() {
    let test_cases = [
        GroupKeyNoteTestCase {
            description: "delete_insert matched by unique_key",
            materialization: MaterializationType::Incremental,
            strategy: Some("delete_insert"),
            has_unique_key: true,
            has_replace_key: false,
            expected_code: Some("DELETE_INSERT_UNIQUE_KEY"),
        },
        GroupKeyNoteTestCase {
            description: "delete_insert matched by replace_key",
            materialization: MaterializationType::Incremental,
            strategy: Some("delete_insert"),
            has_unique_key: false,
            has_replace_key: true,
            expected_code: None,
        },
        GroupKeyNoteTestCase {
            description: "delete_insert by cursor only",
            materialization: MaterializationType::Incremental,
            strategy: Some("delete_insert"),
            has_unique_key: false,
            has_replace_key: false,
            expected_code: None,
        },
        GroupKeyNoteTestCase {
            description: "merge keeps unique_key as its match key",
            materialization: MaterializationType::Incremental,
            strategy: Some("merge"),
            has_unique_key: true,
            has_replace_key: false,
            expected_code: None,
        },
        GroupKeyNoteTestCase {
            description: "a table model never deletes by key",
            materialization: MaterializationType::Table,
            strategy: Some("delete_insert"),
            has_unique_key: true,
            has_replace_key: false,
            expected_code: None,
        },
    ];

    for test_case in test_cases {
        let note = group_key_note(
            "forecast_lines",
            (test_case.materialization, test_case.strategy),
            (test_case.has_unique_key, test_case.has_replace_key),
        );

        assert_eq!(
            note.as_ref()
                .map(|row| (row.code, row.severity, row.message.starts_with("forecast_lines: "))),
            test_case
                .expected_code
                .map(|code| (Some(code), WarningSeverity::Info, true)),
            "{}",
            test_case.description
        );
    }
}
