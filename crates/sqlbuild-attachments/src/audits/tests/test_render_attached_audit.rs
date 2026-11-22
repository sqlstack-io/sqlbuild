use crate::audits::main::render_attached_audit::render_attached_audit;
use crate::audits::models::{ArgumentValue, AuditAttachment, PolicySource};
use crate::audits::tests::helpers::{attachment, failed, rendered};
use crate::audits::tests::test_types::AttachedAuditTestCase;

#[test]
fn given_attached_audits_when_rendering_then_python_rendering_is_returned() {
    let values: (String, ArgumentValue) = (
        "values".to_owned(),
        ArgumentValue::List(vec![ArgumentValue::Number("1".to_owned())]),
    );
    let test_cases = [
        AttachedAuditTestCase {
            description: "explicit arguments merge after implicit ones with default policies",
            attachment: attachment(vec![values.clone()], None, None),
            expected_rendering: rendered(
                "SELECT * FROM t WHERE status NOT IN (1)",
                Ok(("error", PolicySource::Fallback)),
            ),
        },
        AttachedAuditTestCase {
            description: "instance severity and run scope",
            attachment: attachment(vec![values.clone()], Some("warn"), Some("final")),
            expected_rendering: rendered(
                "SELECT * FROM t WHERE status NOT IN (1)",
                Ok(("warn", PolicySource::Instance)),
            ),
        },
        AttachedAuditTestCase {
            description: "an explicit argument overriding an implicit one is Python's error",
            attachment: attachment(
                vec![(
                    "column".to_owned(),
                    ArgumentValue::Text("amount".to_owned()),
                )],
                None,
                None,
            ),
            expected_rendering: failed(
                "models/schema.yml audit 'accepted_values' must not override \
                 implicit column from attached context",
            ),
        },
        AttachedAuditTestCase {
            description: "an explicit argument repeating the implicit text is accepted",
            attachment: attachment(
                vec![
                    (
                        "column".to_owned(),
                        ArgumentValue::Text("status".to_owned()),
                    ),
                    (
                        "values".to_owned(),
                        ArgumentValue::Tuple(vec![
                            ArgumentValue::Text("a".to_owned()),
                            ArgumentValue::Number("inf".to_owned()),
                        ]),
                    ),
                ],
                None,
                None,
            ),
            expected_rendering: rendered(
                "SELECT * FROM t WHERE status NOT IN ('a', inf)",
                Ok(("error", PolicySource::Fallback)),
            ),
        },
        AttachedAuditTestCase {
            description: "an explicit argument repeating the implicit name with a list is an override",
            attachment: attachment(
                vec![(
                    "column".to_owned(),
                    ArgumentValue::List(vec![ArgumentValue::Text("status".to_owned())]),
                )],
                None,
                None,
            ),
            expected_rendering: failed(
                "models/schema.yml audit 'accepted_values' must not override \
                 implicit column from attached context",
            ),
        },
        AttachedAuditTestCase {
            description: "an argument the audit SQL never reads names itself for removal",
            attachment: AuditAttachment {
                definition_path: Some("audits/generic/accepted_values.sql".to_owned()),
                ..attachment(
                    vec![
                        values.clone(),
                        (
                            "where_condition".to_owned(),
                            ArgumentValue::Text("status <> 'void'".to_owned()),
                        ),
                    ],
                    None,
                    None,
                )
            },
            expected_rendering: failed(
                "models/schema.yml audit 'accepted_values' passes argument 'where_condition', \
                 which the audit's SQL in audits/generic/accepted_values.sql never uses; remove \
                 'where_condition' from the attachment",
            ),
        },
        AttachedAuditTestCase {
            description: "a relationships target named as text is told to use a reference",
            attachment: AuditAttachment {
                definition_name: "relationships".to_owned(),
                sql_body: "SELECT * FROM t WHERE status NOT IN (SELECT @field FROM @to)".to_owned(),
                ..attachment(
                    vec![
                        ("to".to_owned(), ArgumentValue::Text("customers".to_owned())),
                        ("field".to_owned(), ArgumentValue::Text("id".to_owned())),
                    ],
                    None,
                    None,
                )
            },
            expected_rendering: failed(
                "models/schema.yml audit 'relationships' argument 'to' is customers, not a \
                 relation reference; write to __ref(\"customers\") (or __source or __seed) so \
                 the audit reads the built relation and runs after it",
            ),
        },
        AttachedAuditTestCase {
            description: "a relationships target written as a reference renders",
            attachment: AuditAttachment {
                definition_name: "relationships".to_owned(),
                sql_body: "SELECT * FROM t WHERE status NOT IN (SELECT @field FROM @to)".to_owned(),
                ..attachment(
                    vec![
                        (
                            "to".to_owned(),
                            ArgumentValue::Text("__ref(\"customers\")".to_owned()),
                        ),
                        ("field".to_owned(), ArgumentValue::Text("id".to_owned())),
                    ],
                    None,
                    None,
                )
            },
            expected_rendering: rendered(
                "SELECT * FROM t WHERE status NOT IN (SELECT id FROM __ref(\"customers\"))",
                Ok(("error", PolicySource::Fallback)),
            ),
        },
        AttachedAuditTestCase {
            description: "a missing argument is Python's error before the policies",
            attachment: attachment(Vec::new(), Some("fatal"), None),
            expected_rendering: failed(
                "models/schema.yml is missing argument 'values' for generic audit \
                 'accepted_values'",
            ),
        },
        AttachedAuditTestCase {
            description: "a seed audit reading @model is told to read @relation",
            attachment: AuditAttachment {
                owner_label: "seeds/waffle_types.yml".to_owned(),
                sql_body: "SELECT * FROM __ref(\"@model\") WHERE NOT (price_cents >= 0)".to_owned(),
                implicit_arguments: vec![
                    ("seed".to_owned(), "waffle_types".to_owned()),
                    ("relation".to_owned(), "__seed(\"waffle_types\")".to_owned()),
                ],
                ..attachment(Vec::new(), None, None)
            },
            expected_rendering: failed(
                "seeds/waffle_types.yml is missing argument 'model' for generic audit \
                 'accepted_values'; a seed audit receives 'seed' and 'relation', not 'model', \
                 so read the attached relation with @relation",
            ),
        },
        AttachedAuditTestCase {
            description: "an opaque value inside a referenced list is Python's value error",
            attachment: attachment(
                vec![(
                    "values".to_owned(),
                    ArgumentValue::List(vec![ArgumentValue::Null, ArgumentValue::Opaque]),
                )],
                None,
                None,
            ),
            expected_rendering: failed(
                "models/schema.yml generic audit 'accepted_values' argument \
                 'values' uses an unsupported value",
            ),
        },
        AttachedAuditTestCase {
            description: "an unknown severity is raised after rendering, before the run scope",
            attachment: attachment(vec![values.clone()], Some("fatal"), Some("delta")),
            expected_rendering: rendered(
                "SELECT * FROM t WHERE status NOT IN (1)",
                Err("models/schema.yml audit \
                 'accepted_values': unknown severity 'fatal'; valid values: warn, error"),
            ),
        },
        AttachedAuditTestCase {
            description: "an unknown run scope lists the sorted valid values",
            attachment: attachment(vec![values.clone()], None, Some("delta")),
            expected_rendering: rendered(
                "SELECT * FROM t WHERE status NOT IN (1)",
                Err("unknown audit run_scope \
                 'delta'; valid values: delta_and_final, final"),
            ),
        },
        AttachedAuditTestCase {
            description: "unknown project defaults name their setting",
            attachment: AuditAttachment {
                default_severity: Some("warn".to_owned()),
                default_run_scope: Some("always".to_owned()),
                ..attachment(vec![values], None, None)
            },
            expected_rendering: rendered(
                "SELECT * FROM t WHERE status NOT IN (1)",
                Err("\
                 settings.default_audit_run_scope in sqlbuild_project.toml: unknown value \
                 'always'; valid values: delta_and_final, final"),
            ),
        },
    ];

    for test_case in test_cases {
        assert_eq!(
            render_attached_audit(&test_case.attachment, None),
            test_case.expected_rendering,
            "{}",
            test_case.description
        );
    }
}
