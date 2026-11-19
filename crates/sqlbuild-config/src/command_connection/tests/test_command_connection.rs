use crate::command_connection::errors::CommandConnectionError;
use crate::command_connection::main::authored_warehouse::authored_warehouse;
use crate::command_connection::main::check_cli_warehouse::check_cli_warehouse;
use crate::command_connection::main::check_expanded_warehouse::check_expanded_warehouse;
use crate::command_connection::main::check_job_warehouse::check_job_warehouse;
use crate::command_connection::main::snowflake_key_warning::snowflake_key_warning;
use crate::command_connection::tests::helpers::message;
use crate::command_connection::tests::test_types::{
    CliWarehouseTestCase, ExpandedWarehouseTestCase, JobWarehouseTestCase, SnowflakeKeyTestCase,
};

#[test]
fn given_cli_warehouse_when_checking_then_matches_python_identifier_rules() {
    let test_cases = [
        CliWarehouseTestCase {
            description: "unquoted identifier with dollar",
            cli_warehouse: "ADHOC_WH$1",
            expected_message: None,
        },
        CliWarehouseTestCase {
            description: "quoted identifier with doubled quotes",
            cli_warehouse: "\"Adhoc \"\" WH\"",
            expected_message: None,
        },
        CliWarehouseTestCase {
            description: "blank value",
            cli_warehouse: "  ",
            expected_message: Some("--warehouse requires a warehouse name"),
        },
        CliWarehouseTestCase {
            description: "lone quote inside a quoted identifier",
            cli_warehouse: "\"a\"b\"",
            expected_message: Some(
                "--warehouse '\"a\"b\"' is not a valid Snowflake warehouse identifier",
            ),
        },
        CliWarehouseTestCase {
            description: "empty quoted identifier",
            cli_warehouse: "\"\"",
            expected_message: Some(
                "--warehouse '\"\"' is not a valid Snowflake warehouse identifier",
            ),
        },
        CliWarehouseTestCase {
            description: "space in an unquoted identifier",
            cli_warehouse: "bad wh",
            expected_message: Some(
                "--warehouse 'bad wh' is not a valid Snowflake warehouse identifier",
            ),
        },
        CliWarehouseTestCase {
            description: "non-ASCII letter",
            cli_warehouse: "wä",
            expected_message: Some(
                "--warehouse 'wä' is not a valid Snowflake warehouse identifier",
            ),
        },
    ];

    for test_case in test_cases {
        assert_eq!(
            message(check_cli_warehouse(test_case.cli_warehouse)).as_deref(),
            test_case.expected_message,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_expanded_group_warehouse_when_checking_then_invalid_values_show_the_setting() {
    let test_cases = [
        ExpandedWarehouseTestCase {
            description: "project value that is not an identifier",
            local_value: None,
            project_value: Some("${wh}"),
            expanded: "bad wh",
            expected_result: Err(CommandConnectionError::User {
                code: Some("C261"),
                message: "sqlbuild_project.toml [targets.dev.warehouses] build resolves to 'bad wh', which is not a valid Snowflake warehouse identifier".to_owned(),
                help: Some("to use an unquoted identifier (or a double-quoted one such as \"Adhoc WH\"), set this in sqlbuild_project.toml:\n            [targets.dev.warehouses]\n            build = \"ADHOC_WH\"".to_owned()),
            }),
        },
        ExpandedWarehouseTestCase {
            description: "local value wins and is valid",
            local_value: Some("LOCAL_WH"),
            project_value: Some("bad wh"),
            expanded: "LOCAL_WH",
            expected_result: Ok(()),
        },
    ];

    for test_case in test_cases {
        let result: Result<(), CommandConnectionError> = authored_warehouse(
            ("dev", "build"),
            test_case.local_value.map(str::to_owned),
            test_case.project_value.map(str::to_owned),
        )
        .map_or(Ok(()), |authored| {
            check_expanded_warehouse(&authored, test_case.expanded)
        });
        assert_eq!(
            result, test_case.expected_result,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_connection_keys_when_warning_then_only_duckdb_snowflake_keys_are_listed() {
    let test_cases = [
        SnowflakeKeyTestCase {
            description: "sorted snowflake keys on duckdb",
            adapter_name: "duckdb",
            keys: &["warehouse", "database", "account"],
            expected_warning: Some(
                "Warning: DuckDB adapter is active, but connection contains Snowflake-like keys: account, warehouse. If this is a Snowflake local config, add `adapter = \"snowflake\"` at the top of sqlbuild_local.toml, before any section.",
            ),
        },
        SnowflakeKeyTestCase {
            description: "snowflake adapter never warns",
            adapter_name: "snowflake",
            keys: &["account"],
            expected_warning: None,
        },
    ];

    for test_case in test_cases {
        let keys: Vec<String> = test_case.keys.iter().map(|key| (*key).to_owned()).collect();
        assert_eq!(
            snowflake_key_warning(test_case.adapter_name, &keys).as_deref(),
            test_case.expected_warning,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_a_job_warehouse_when_checking_then_it_names_the_job_and_follows_identifier_rules() {
    let test_cases = [
        JobWarehouseTestCase {
            description: "unquoted identifier",
            value: Some("ORDERS_WH"),
            expected_name: Some("ORDERS_WH"),
            expected_message: None,
        },
        JobWarehouseTestCase {
            description: "blank value",
            value: Some(" "),
            expected_name: None,
            expected_message: Some("job orders_job sets an empty warehouse name"),
        },
        JobWarehouseTestCase {
            description: "not an identifier",
            value: Some("bad wh"),
            expected_name: None,
            expected_message: Some(
                "job orders_job sets warehouse 'bad wh', which is not a valid Snowflake warehouse identifier",
            ),
        },
        JobWarehouseTestCase {
            description: "not text",
            value: None,
            expected_name: None,
            expected_message: Some("job orders_job sets a warehouse that is not text"),
        },
    ];

    for test_case in test_cases {
        let result = check_job_warehouse("orders_job", test_case.value);
        let name: Option<&str> = result.as_deref().ok();
        let message: Option<&str> = result.as_ref().err().map(|error| error.message());
        assert_eq!(
            (name, message),
            (test_case.expected_name, test_case.expected_message),
            "{}",
            test_case.description
        );
    }
}
