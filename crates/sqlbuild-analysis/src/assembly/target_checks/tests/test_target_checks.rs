use crate::assembly::target_checks::errors::TargetError;
use crate::assembly::target_checks::main::explicit_write_schema::explicit_write_schema;
use crate::assembly::target_checks::main::first_shared_namespace::first_shared_namespace;
use crate::assembly::target_checks::main::target_error_message::target_error_message;
use crate::assembly::target_checks::models::{LoaderCollision, MissingParts};
use crate::assembly::target_checks::tests::helpers::owned;
use crate::assembly::target_checks::tests::test_types::{
    MessageTestCase, NamespaceTestCase, SchemaTestCase,
};

#[test]
fn given_target_error_when_formatting_then_matches_python_message() {
    let test_cases = [
        MessageTestCase {
            description: "a seed missing both parts",
            error: TargetError::MissingParts(MissingParts {
                adapter_name: "snowflake".to_owned(),
                resource_kind: "seed".to_owned(),
                resource_name: "products".to_owned(),
                database: None,
                schema: None,
            }),
            expected_message: "snowflake execution requires explicit target database, schema. seed 'products' resolved to database=None schema=None. Set them in the model config, the environment config, or the project defaults.",
        },
        MessageTestCase {
            description: "a model missing its schema, with a quoted database",
            error: TargetError::MissingParts(MissingParts {
                adapter_name: "bigquery".to_owned(),
                resource_kind: "model".to_owned(),
                resource_name: "orders".to_owned(),
                database: owned(Some("it's")),
                schema: None,
            }),
            expected_message: "bigquery execution requires explicit target schema. model 'orders' resolved to database=\"it's\" schema=None. Set them in the model config, the environment config, or the project defaults.",
        },
        MessageTestCase {
            description: "a loader collision",
            error: TargetError::LoaderCollision(LoaderCollision {
                left_target: "dev".to_owned(),
                right_target: "prod".to_owned(),
                database: None,
                schema: owned(Some("raw")),
            }),
            expected_message: "Managed loader write collision: targets 'dev' and 'prod' both resolve to database=None schema='raw'. Set distinct targets.dev.loader_schema and targets.prod.loader_schema values.",
        },
        MessageTestCase {
            description: "a managed write without a schema",
            error: TargetError::ManagedWriteSchema("Model 'orders'".to_owned()),
            expected_message: "Model 'orders' has no explicitly resolved physical write schema. Set schema on the resource or its defaults, selected target, loader_schema, or connection.",
        },
        MessageTestCase {
            description: "a named target without a schema",
            error: TargetError::NamedTargetSchema("ci".to_owned()),
            expected_message: "Named target 'ci' must explicitly set schema to a nonblank literal or 'preserve'. Resource schemas, defaults, and connection schemas do not select a named target schema strategy.",
        },
        MessageTestCase {
            description: "an intermediate loader under a preserved target schema",
            error: TargetError::PreservedLoaderSchema("raw_orders".to_owned()),
            expected_message: "Intermediate loader 'raw_orders' would write to a schema named 'preserve'. The target's schema 'preserve' keeps each model's own schema, and a loader has none, so its destination must name a schema.",
        },
    ];

    for test_case in test_cases {
        let message = target_error_message(&test_case.error);

        assert_eq!(
            message, test_case.expected_message,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_schema_when_reading_explicit_write_schema_then_matches_python() {
    let test_cases = [
        SchemaTestCase {
            description: "a padded schema is stripped",
            schema: Some(" analytics\t"),
            expected_schema: Some("analytics"),
        },
        SchemaTestCase {
            description: "preserve is not a physical schema",
            schema: Some(" preserve "),
            expected_schema: None,
        },
        SchemaTestCase {
            description: "a blank schema is none",
            schema: Some("\u{3000}"),
            expected_schema: None,
        },
        SchemaTestCase {
            description: "no schema is none",
            schema: None,
            expected_schema: None,
        },
    ];

    for test_case in test_cases {
        let schema = explicit_write_schema(test_case.schema);

        assert_eq!(
            schema.as_deref(),
            test_case.expected_schema,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_shared_namespaces_when_choosing_first_then_sorts_by_python_repr() {
    let test_cases = [
        NamespaceTestCase {
            description: "a quoted string sorts before None",
            namespaces: &[(None, Some("raw")), (Some("db"), Some("raw"))],
            expected_namespace: Some((Some("db"), Some("raw"))),
        },
        NamespaceTestCase {
            description: "no namespace is shared",
            namespaces: &[],
            expected_namespace: None,
        },
    ];

    for test_case in test_cases {
        let namespaces: Vec<(Option<String>, Option<String>)> = test_case
            .namespaces
            .iter()
            .map(|(database, schema)| (owned(*database), owned(*schema)))
            .collect();

        let first = first_shared_namespace(&namespaces);

        assert_eq!(
            first,
            test_case
                .expected_namespace
                .map(|(database, schema)| (owned(database), owned(schema))),
            "{}",
            test_case.description
        );
    }
}
