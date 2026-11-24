use crate::declarations::errors::DeclarationFailure;
use crate::declarations::main::model_enum_declarations::model_enum_declarations;
use crate::declarations::models::DeclarationSite;
use crate::declarations::tests::helpers::enums;
use crate::declarations::tests::test_types::HeaderEnumTestCase;
use crate::sql_values::models::{RawKey, RawSqlValue};

#[test]
fn given_model_header_enums_when_validating_then_python_rules_apply() {
    let test_cases = [
        HeaderEnumTestCase {
            description: "shorthand members are text",
            raw: enums(
                "_status",
                RawSqlValue::List(vec![RawSqlValue::Str("ACTIVE".to_owned())]),
            ),
            expected_scalar_types: Ok(vec!["VARCHAR"]),
        },
        HeaderEnumTestCase {
            description: "explicit integer members",
            raw: enums(
                "_status",
                RawSqlValue::Object(vec![(
                    RawKey::Str("ACTIVE".to_owned()),
                    RawSqlValue::Int(1),
                )]),
            ),
            expected_scalar_types: Ok(vec!["INTEGER"]),
        },
        HeaderEnumTestCase {
            description: "model-local enums must be private snake_case identities",
            raw: enums(
                "Status",
                RawSqlValue::List(vec![RawSqlValue::Str("ACTIVE".to_owned())]),
            ),
            expected_scalar_types: Err(DeclarationFailure::Identity(
                "Invalid model-local enum identity 'Status' in sql/models/a.sql; use snake_case \
                 '_status'"
                    .to_owned(),
                crate::header_metadata::constants::IDENTITY_HELP.to_owned(),
            )),
        },
        HeaderEnumTestCase {
            description: "member identifiers are uppercase",
            raw: enums(
                "_status",
                RawSqlValue::List(vec![RawSqlValue::Str("active".to_owned())]),
            ),
            expected_scalar_types: Err(DeclarationFailure::Parse(
                "sql/models/a.sql enum '_status' member identifiers must be uppercase: 'active'"
                    .to_owned(),
            )),
        },
    ];

    for test_case in test_cases {
        let site = DeclarationSite {
            file_path: "sql/models/a.sql",
            model_name: Some("a"),
        };

        let declared = model_enum_declarations(Some(&test_case.raw), site).map(|facts| {
            facts
                .iter()
                .map(|fact| fact.scalar_type)
                .collect::<Vec<_>>()
        });

        assert_eq!(
            declared, test_case.expected_scalar_types,
            "{}",
            test_case.description
        );
    }
}
