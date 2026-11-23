use crate::yaml_entries::tests::helpers::{column, map, schema_outcome, seed, text};
use crate::yaml_entries::tests::test_types::{Node, SchemaDocumentTestCase, lines, parse_error};

#[test]
fn given_schema_documents_when_parsing_then_matches_python_seeds_and_errors() {
    let test_cases = [
        SchemaDocumentTestCase {
            description: "a valid seed keeps its columns and tags",
            document: seed(
                vec![column("code", "VARCHAR"), column("label", "VARCHAR")],
                vec![("tags", Node::List(vec![text("reference")]))],
            ),
            in_seeds_directory: true,
            expected_seeds: lines(&["countries columns=2 tags=reference csv_settings=false"]),
        },
        SchemaDocumentTestCase {
            description: "a document that is not a mapping",
            document: Node::List(Vec::new()),
            in_seeds_directory: true,
            expected_seeds: parse_error(
                "/project/seeds/countries.yml must contain a top-level mapping",
            ),
        },
        SchemaDocumentTestCase {
            description: "model metadata in YAML",
            document: map(vec![(
                "models",
                Node::List(vec![map(vec![("name", text("orders"))])]),
            )]),
            in_seeds_directory: false,
            expected_seeds: parse_error(
                "/project/seeds/countries.yml declares model metadata in 'models', but model \
                 metadata must live in the model file MODEL(...). Move description, columns, and \
                 audits into the model header.",
            ),
        },
        SchemaDocumentTestCase {
            description: "seeds outside the seeds directory",
            document: seed(vec![column("code", "VARCHAR")], Vec::new()),
            in_seeds_directory: false,
            expected_seeds: parse_error(
                "/project/seeds/countries.yml declares seeds, but seed declarations must live \
                 under sql/seeds/**/*.yml",
            ),
        },
        SchemaDocumentTestCase {
            description: "a seed column without a type",
            document: seed(vec![map(vec![("name", text("code"))])], Vec::new()),
            in_seeds_directory: true,
            expected_seeds: parse_error(
                "/project/seeds/countries.yml seed column 'code' must define non-empty string 'type'",
            ),
        },
        SchemaDocumentTestCase {
            description: "null audits are not an empty list",
            document: seed(
                vec![column("code", "VARCHAR")],
                vec![("audits", Node::Null)],
            ),
            in_seeds_directory: true,
            expected_seeds: parse_error("/project/seeds/countries.yml seed audits must be a list"),
        },
        SchemaDocumentTestCase {
            description: "an audit name Python strips to nothing",
            document: seed(
                vec![column("code", "VARCHAR")],
                vec![("audits", Node::List(vec![text("\u{1f}")]))],
            ),
            in_seeds_directory: true,
            expected_seeds: parse_error(
                "/project/seeds/countries.yml seed audits must not contain empty names",
            ),
        },
        SchemaDocumentTestCase {
            description: "a non-boolean csv setting",
            document: seed(
                vec![column("code", "VARCHAR")],
                vec![("csv_settings", map(vec![("doublequote", Node::Int(1))]))],
            ),
            in_seeds_directory: true,
            expected_seeds: parse_error(
                "/project/seeds/countries.yml seed csv_settings 'doublequote' must be a boolean",
            ),
        },
    ];
    for test_case in test_cases {
        assert_eq!(
            schema_outcome(&test_case.document, test_case.in_seeds_directory),
            test_case.expected_seeds,
            "{}",
            test_case.description
        );
    }
}
