use crate::definition_locations::main::decorated_functions::decorated_functions;
use crate::definition_locations::tests::test_types::DecoratedFunctionsTestCase;

#[test]
fn given_module_when_listing_decorated_functions_then_only_top_level_functions_count() {
    let test_cases = [
        DecoratedFunctionsTestCase {
            description: "an attribute call decorator",
            contents: "import sqlbuild\n\n@sqlbuild.loader(name=\"orders\")\ndef load():\n    pass\n\n\ndef plain():\n    pass\n",
            expected_functions: &["load"],
        },
        DecoratedFunctionsTestCase {
            description: "a bare imported decorator among others",
            contents: "@cache\n@task\ndef run():\n    pass\n\n\n@asset\ndef build():\n    pass\n",
            expected_functions: &["run", "build"],
        },
        DecoratedFunctionsTestCase {
            description: "a decorated method is not a top-level function",
            contents: "class Box:\n    @task\n    def run(self):\n        pass\n",
            expected_functions: &[],
        },
        DecoratedFunctionsTestCase {
            description: "a nested function is not top level",
            contents: "def outer():\n    @asset\n    def inner():\n        pass\n",
            expected_functions: &[],
        },
        DecoratedFunctionsTestCase {
            description: "an unrelated decorator",
            contents: "@udf\ndef add_one(value):\n    return value + 1\n",
            expected_functions: &[],
        },
        DecoratedFunctionsTestCase {
            description: "a syntax error",
            contents: "@task\ndef run(:\n",
            expected_functions: &[],
        },
    ];
    for test_case in test_cases {
        assert_eq!(
            decorated_functions(test_case.contents, &["loader", "task", "asset"], (3, 12)),
            test_case.expected_functions,
            "{}",
            test_case.description
        );
    }
}
