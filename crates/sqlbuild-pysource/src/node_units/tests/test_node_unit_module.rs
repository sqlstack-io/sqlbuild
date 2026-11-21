use crate::node_units::models::{UnitFunction, UnitImport, UnitStatementKind};
use crate::node_units::tests::helpers::{
    at, decorator, import, imports, last_statement, names, parameter, statement_kinds,
};
use crate::node_units::tests::test_types::{
    FunctionTestCase, ImportsTestCase, StatementKindsTestCase,
};

const CRLF_NODE_MODULE: &str = "\"\"\"Doc.\"\"\"\r\nfrom .main import main\r\nfrom python.orders.daily.node import daily as upstream\r\nimport python.utils.text\r\n\r\nX = 'é'\r\n\r\n\r\n@sqlbuild.asset(depends_on=[upstream], description=\"D.\")\r\ndef run(ctx: AssetContext, /, *, store: \"providers.Store\") -> PythonNodeResult | None:\r\n    \"\"\"Doc.\"\"\"\r\n    return main(ctx=ctx, store=store)\r\n";

const FACTORY_MODULE: &str = "@factory\ndef tables():\n    for table in TABLES:\n        for _ in specs():\n            pass\n\n        @asset(name=table)\n        def load(ctx, *args, **kwargs) -> Optional[Union[LoaderResult, list]]:\n            rows = main(ctx=ctx)\n            return rows\n\n    def plain():\n        return main(ctx, **extra)\n";

#[test]
fn given_module_when_reading_then_each_top_level_statement_has_its_kind() {
    let test_cases = [
        StatementKindsTestCase {
            description: "a CRLF node module with a docstring, imports, an assignment and a node",
            contents: CRLF_NODE_MODULE,
            expected_kinds: Some(vec![
                "Docstring",
                "Import",
                "Import",
                "Import",
                "Assignment",
                "Function",
            ]),
        },
        StatementKindsTestCase {
            description: "a class and a loop",
            contents: "class Store(Provider):\n    pass\n\n\nfor row in rows:\n    pass\n",
            expected_kinds: Some(vec!["Class", "Other"]),
        },
        StatementKindsTestCase {
            description: "a module that does not parse",
            contents: "def broken(:\n",
            expected_kinds: None,
        },
    ];
    for test_case in test_cases {
        assert_eq!(
            statement_kinds(test_case.contents),
            test_case
                .expected_kinds
                .map(|kinds| kinds.into_iter().map(str::to_owned).collect()),
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_imports_when_reading_then_each_bound_name_records_whether_only_depends_on_uses_it() {
    let test_cases = [ImportsTestCase {
        description: "a relative import, an aliased node used in depends_on and an unused dotted import",
        contents: CRLF_NODE_MODULE,
        expected_imports: vec![
            UnitImport {
                level: 1,
                ..import("main", Some("main"), "main", at(2, 19))
            },
            UnitImport {
                only_in_depends_on: true,
                ..import(
                    "python.orders.daily.node",
                    Some("daily"),
                    "upstream",
                    at(3, 38),
                )
            },
            import("python.utils.text", None, "python", at(4, 8)),
        ],
    }];
    for test_case in test_cases {
        assert_eq!(
            imports(test_case.contents),
            test_case.expected_imports,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_declaration_when_reading_then_signature_body_and_inner_declarations_are_read() {
    let test_cases = [
        FunctionTestCase {
            description: "a node with positional-only ctx, a quoted provider and a union result",
            contents: CRLF_NODE_MODULE,
            expected_function: UnitFunction {
                name: "run".to_owned(),
                position: at(10, 5),
                decorators: vec![decorator(
                    "asset",
                    &["depends_on", "description"],
                    &["upstream"],
                )],
                positional: vec![parameter("ctx", Some("AssetContext"))],
                variadic: false,
                keyword_only: vec![parameter("store", Some("Store"))],
                keywords_variadic: false,
                returns: Some(names(&["PythonNodeResult", "None"])),
                docstring: true,
                returns_directly: true,
                delegation: Some(true),
                inner: vec![],
                loops: vec![],
            },
        },
        FunctionTestCase {
            description: "a factory looping over a name and a call, whose generated node computes before returning",
            contents: FACTORY_MODULE,
            expected_function: UnitFunction {
                name: "tables".to_owned(),
                position: at(2, 5),
                decorators: vec![decorator("factory", &[], &[])],
                positional: vec![],
                variadic: false,
                keyword_only: vec![],
                keywords_variadic: false,
                returns: None,
                docstring: false,
                returns_directly: false,
                delegation: None,
                inner: vec![UnitFunction {
                    name: "load".to_owned(),
                    position: at(8, 13),
                    decorators: vec![decorator("asset", &["name"], &[])],
                    positional: vec![parameter("ctx", None)],
                    variadic: true,
                    keyword_only: vec![],
                    keywords_variadic: true,
                    returns: Some(names(&["LoaderResult", "list", "None"])),
                    docstring: false,
                    returns_directly: false,
                    delegation: None,
                    inner: vec![],
                    loops: vec![],
                }],
                loops: vec![names(&["TABLES"]), names(&["specs"])],
            },
        },
        FunctionTestCase {
            description: "a body that calls main() with a positional argument and unpacking",
            contents: "def run(ctx):\n    return main(ctx, **extra)\n",
            expected_function: UnitFunction {
                name: "run".to_owned(),
                position: at(1, 5),
                decorators: vec![],
                positional: vec![parameter("ctx", None)],
                variadic: false,
                keyword_only: vec![],
                keywords_variadic: false,
                returns: None,
                docstring: false,
                returns_directly: true,
                delegation: Some(false),
                inner: vec![],
                loops: vec![],
            },
        },
    ];
    for test_case in test_cases {
        assert_eq!(
            last_statement(test_case.contents),
            Some(UnitStatementKind::Function(test_case.expected_function)),
            "{}",
            test_case.description
        );
    }
}
