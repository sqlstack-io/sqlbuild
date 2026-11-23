use crate::target_paths::main::function_run_path::function_run_path;
use crate::target_paths::main::model_run_path::model_run_path;
use crate::target_paths::main::runtime_sql_text::runtime_sql_text;
use crate::target_paths::main::sql_test_output_path::sql_test_output_path;
use crate::target_paths::tests::helpers::facts;
use crate::target_paths::tests::test_types::{
    RunPathTestCase, RuntimeSqlTestCase, SqlTestPathTestCase,
};

/// Expectations recorded from the retired Python path builders.
#[test]
fn given_sql_test_when_building_output_path_then_matches_python() {
    let long: Vec<String> = (0..9)
        .map(|index| format!("m\u{e9}tric_{index:02}_xxxxxxxxxxxxxxxxxxxx"))
        .collect();
    let last: String = format!("{}a", "\u{e9}".repeat(50));
    let mut long_chain: Vec<&str> = vec![last.as_str()];
    long_chain.extend(long.iter().map(String::as_str));
    let test_cases = [
        SqlTestPathTestCase {
            description: "test without models uses its own name",
            facts: facts("t1", None, &[]),
            expected_path: "t1/t1.sql",
        },
        SqlTestPathTestCase {
            description: "one repeated model names the folder",
            facts: facts("t2", None, &["orders", "orders"]),
            expected_path: "orders/t2.sql",
        },
        SqlTestPathTestCase {
            description: "a chain is sorted and deduplicated",
            facts: facts("t3", None, &["zeta", "alpha", "zeta"]),
            expected_path: "_chain_/alpha__zeta/t3.sql",
        },
        SqlTestPathTestCase {
            description: "a long chain keeps truncated edge names and a digest",
            facts: facts("t4", None, &long_chain),
            expected_path: "_chain_/m\u{e9}tric_00_xxxxxxxxxxxxxxxxxxxx__\u{e9}\u{e9}\u{e9}\u{e9}\u{e9}\u{e9}\u{e9}\u{e9}\u{e9}\u{e9}\u{e9}\u{e9}\u{e9}\u{e9}\u{e9}\u{e9}\u{e9}\u{e9}\u{e9}\u{e9}\u{e9}\u{e9}\u{e9}\u{e9}\u{e9}\u{e9}\u{e9}\u{e9}\u{e9}\u{e9}\u{e9}\u{e9}\u{e9}\u{e9}\u{e9}\u{e9}\u{e9}\u{e9}\u{e9}\u{e9}__132c41e73f1c/t4.sql",
        },
        SqlTestPathTestCase {
            description: "an expanded case lives under its source stem",
            facts: facts(
                "t5",
                Some(("tests/orders.sql", Some(2), "happy")),
                &["orders"],
            ),
            expected_path: "tests/orders/block_2__happy.sql",
        },
        SqlTestPathTestCase {
            description: "a dot file keeps its name and a missing block prints None",
            facts: facts("t6", Some(("tests/.hidden", None, "c")), &[]),
            expected_path: "tests/.hidden/block_None__c.sql",
        },
    ];

    for test_case in test_cases {
        assert_eq!(
            sql_test_output_path(&test_case.facts),
            test_case.expected_path,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_relative_path_when_building_run_paths_then_matches_python() {
    let test_cases = [
        RunPathTestCase {
            description: "paths already under their directory are kept",
            relative_path: "sql/models/a/b.sql",
            language: "sql",
            expected_model_path: "sql/models/a/b.sql",
            expected_function_path: "sql/functions/sql/sql/models/a/b.sql",
        },
        RunPathTestCase {
            description: "other paths are nested and functions get a sql suffix",
            relative_path: "udfs/f.py",
            language: "python",
            expected_model_path: "sql/models/udfs/f.py",
            expected_function_path: "sql/functions/python/udfs/f.sql",
        },
        RunPathTestCase {
            description: "a function under its language directory is kept",
            relative_path: "sql/functions/sql/f.sql",
            language: "sql",
            expected_model_path: "sql/models/sql/functions/sql/f.sql",
            expected_function_path: "sql/functions/sql/f.sql",
        },
    ];

    for test_case in test_cases {
        assert_eq!(
            (
                model_run_path(test_case.relative_path),
                function_run_path(test_case.relative_path, test_case.language)
            ),
            (
                test_case.expected_model_path.to_owned(),
                test_case.expected_function_path.to_owned()
            ),
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_statements_when_rendering_runtime_sql_then_matches_python() {
    let test_cases = [RuntimeSqlTestCase {
        description: "statements are terminated and blank ones kept",
        statements: vec![
            "select 1  ".to_owned(),
            "create table x as select 2;\n".to_owned(),
            "   ".to_owned(),
            "drop view y\u{1f}".to_owned(),
        ],
        expected_text: "select 1;\n\ncreate table x as select 2;\n\n   \n\ndrop view y;\n",
    }];

    for test_case in test_cases {
        assert_eq!(
            runtime_sql_text(&test_case.statements),
            test_case.expected_text,
            "{}",
            test_case.description
        );
    }
}
