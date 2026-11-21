use crate::functions::main::parse_function_header::parse_function_header;
use crate::functions::models::{
    FunctionHeader, FunctionLanguage, FunctionReturns, HeaderStage, HeaderValue,
};
use crate::functions::tests::helpers::{empty_header, failure, header, map, named, text};
use crate::functions::tests::test_types::FunctionHeaderTestCase;

#[test]
fn given_function_headers_when_parsing_then_python_values_and_first_error_are_returned() {
    let test_cases = [
        FunctionHeaderTestCase {
            description: "a SQL table function with stripped arguments, columns and tags",
            header: header(&[
                ("arguments", map(&[(" raw_status ", text(" STRING "))])),
                (
                    "returns",
                    map(&[("table", map(&[("id", text("INTEGER"))]))]),
                ),
                ("tags", HeaderValue::Sequence(vec![text(" orders ")])),
                ("description", text(" Order rows. ")),
            ]),
            language: FunctionLanguage::Sql,
            table_function: false,
            expected_header: FunctionHeader {
                arguments: vec![named(" raw_status ", "raw_status", "STRING")],
                returns: Some(FunctionReturns::Table(vec![named("id", "id", "INTEGER")])),
                tags: vec!["orders".to_owned()],
                description: Some("Order rows.".to_owned()),
                ..empty_header()
            },
        },
        FunctionHeaderTestCase {
            description: "a Python function with its runtime, entry point and packages",
            header: header(&[
                ("returns", text("STRING")),
                ("runtime_version", text("3.12")),
                ("entry_point", text(" label ")),
                ("packages", HeaderValue::Sequence(vec![text("numpy")])),
            ]),
            language: FunctionLanguage::Python,
            table_function: false,
            expected_header: FunctionHeader {
                returns: Some(FunctionReturns::Type("STRING".to_owned())),
                runtime_version: Some("3.12".to_owned()),
                entry_point: Some("label".to_owned()),
                packages: vec!["numpy".to_owned()],
                ..empty_header()
            },
        },
        FunctionHeaderTestCase {
            description: "a missing return type is raised before the arguments",
            header: header(&[("arguments", HeaderValue::Other)]),
            language: FunctionLanguage::Sql,
            table_function: false,
            expected_header: FunctionHeader {
                failure: failure(
                    HeaderStage::Start,
                    "SQL function file functions/f.sql must declare returns",
                ),
                ..empty_header()
            },
        },
        FunctionHeaderTestCase {
            description: "a blank argument type keeps the arguments before it",
            header: header(&[
                ("returns", HeaderValue::Other),
                (
                    "arguments",
                    map(&[("id", text("INTEGER")), (" value ", text("  "))]),
                ),
            ]),
            language: FunctionLanguage::Sql,
            table_function: false,
            expected_header: FunctionHeader {
                arguments: vec![named("id", "id", "INTEGER")],
                failure: failure(
                    HeaderStage::Arguments,
                    "SQL function file functions/f.sql argument ' value ' must declare a type",
                ),
                ..empty_header()
            },
        },
        FunctionHeaderTestCase {
            description: "a blank SQL return type is a shape error after the arguments",
            header: header(&[("returns", text(" ")), ("tags", HeaderValue::Other)]),
            language: FunctionLanguage::Sql,
            table_function: false,
            expected_header: FunctionHeader {
                returns: Some(FunctionReturns::Table(Vec::new())),
                failure: failure(
                    HeaderStage::Returns,
                    "SQL function file functions/f.sql returns must be a type string or table \
                     column declaration",
                ),
                ..empty_header()
            },
        },
        FunctionHeaderTestCase {
            description: "a return column without a name keeps the columns before it",
            header: header(&[(
                "returns",
                map(&[(
                    "table",
                    HeaderValue::Map(vec![
                        (text("id"), text("INTEGER")),
                        (HeaderValue::Other, text("TEXT")),
                    ]),
                )]),
            )]),
            language: FunctionLanguage::Sql,
            table_function: false,
            expected_header: FunctionHeader {
                returns: Some(FunctionReturns::Table(vec![named("id", "id", "INTEGER")])),
                failure: failure(
                    HeaderStage::Returns,
                    "SQL function file functions/f.sql has an invalid return column name",
                ),
                ..empty_header()
            },
        },
        FunctionHeaderTestCase {
            description: "an empty table is Python's column error",
            header: header(&[("returns", map(&[("table", map(&[]))]))]),
            language: FunctionLanguage::Sql,
            table_function: false,
            expected_header: FunctionHeader {
                returns: Some(FunctionReturns::Table(Vec::new())),
                failure: failure(
                    HeaderStage::Returns,
                    "SQL function file functions/f.sql returns table must declare at least one \
                     column",
                ),
                ..empty_header()
            },
        },
        FunctionHeaderTestCase {
            description: "a Python function returning a table must declare returns",
            header: header(&[
                (
                    "returns",
                    map(&[("table", map(&[("id", text("INTEGER"))]))]),
                ),
                ("runtime_version", text("3.12")),
                ("entry_point", text("label")),
            ]),
            language: FunctionLanguage::Python,
            table_function: false,
            expected_header: FunctionHeader {
                failure: failure(
                    HeaderStage::Start,
                    "Python function file functions/f.sql must declare returns",
                ),
                ..empty_header()
            },
        },
        FunctionHeaderTestCase {
            description: "a missing entry point is raised after the runtime version",
            header: header(&[
                ("returns", text("STRING")),
                ("runtime_version", text("3.12")),
                ("packages", HeaderValue::Other),
            ]),
            language: FunctionLanguage::Python,
            table_function: false,
            expected_header: FunctionHeader {
                returns: Some(FunctionReturns::Type("STRING".to_owned())),
                runtime_version: Some("3.12".to_owned()),
                failure: failure(
                    HeaderStage::PythonValues,
                    "Python function file functions/f.sql must declare entry_point",
                ),
                ..empty_header()
            },
        },
        FunctionHeaderTestCase {
            description: "a blank package entry and then the tags",
            header: header(&[
                ("returns", text("STRING")),
                ("runtime_version", text("3.12")),
                ("entry_point", text("label")),
                ("packages", HeaderValue::Sequence(vec![text("")])),
            ]),
            language: FunctionLanguage::Python,
            table_function: false,
            expected_header: FunctionHeader {
                returns: Some(FunctionReturns::Type("STRING".to_owned())),
                runtime_version: Some("3.12".to_owned()),
                entry_point: Some("label".to_owned()),
                failure: failure(
                    HeaderStage::PythonValues,
                    "Python function file functions/f.sql packages entries must be non-empty \
                     strings",
                ),
                ..empty_header()
            },
        },
        FunctionHeaderTestCase {
            description: "tags that are not a list are raised before the description",
            header: header(&[
                ("returns", text("STRING")),
                ("tags", text("orders")),
                ("description", HeaderValue::Other),
            ]),
            language: FunctionLanguage::Sql,
            table_function: false,
            expected_header: FunctionHeader {
                returns: Some(FunctionReturns::Type("STRING".to_owned())),
                failure: failure(
                    HeaderStage::Metadata,
                    "SQL function file functions/f.sql tags must be a list",
                ),
                ..empty_header()
            },
        },
        FunctionHeaderTestCase {
            description: "a description that is not a string",
            header: header(&[
                ("returns", text("STRING")),
                ("description", HeaderValue::Other),
            ]),
            language: FunctionLanguage::Sql,
            table_function: false,
            expected_header: FunctionHeader {
                returns: Some(FunctionReturns::Type("STRING".to_owned())),
                failure: failure(
                    HeaderStage::Metadata,
                    "SQL function file functions/f.sql description must be a string",
                ),
                ..empty_header()
            },
        },
        FunctionHeaderTestCase {
            description: "a Python table function maps each return column to its type",
            header: header(&[
                (
                    "returns",
                    map(&[(" ts ", text(" FLOAT ")), ("value", text("FLOAT"))]),
                ),
                ("runtime_version", text("3.12")),
                ("entry_point", text("main")),
            ]),
            language: FunctionLanguage::Python,
            table_function: true,
            expected_header: FunctionHeader {
                returns: Some(FunctionReturns::Table(vec![
                    named(" ts ", "ts", "FLOAT"),
                    named("value", "value", "FLOAT"),
                ])),
                runtime_version: Some("3.12".to_owned()),
                entry_point: Some("main".to_owned()),
                ..empty_header()
            },
        },
        FunctionHeaderTestCase {
            description: "a Python table function with a scalar return type",
            header: header(&[("returns", text("FLOAT"))]),
            language: FunctionLanguage::Python,
            table_function: true,
            expected_header: FunctionHeader {
                returns: Some(FunctionReturns::Table(Vec::new())),
                failure: failure(
                    HeaderStage::Returns,
                    "Python function file functions/f.sql returns must map each column name to its type",
                ),
                ..empty_header()
            },
        },
        FunctionHeaderTestCase {
            description: "a Python table function with no return columns",
            header: header(&[("returns", map(&[]))]),
            language: FunctionLanguage::Python,
            table_function: true,
            expected_header: FunctionHeader {
                returns: Some(FunctionReturns::Table(Vec::new())),
                failure: failure(
                    HeaderStage::Returns,
                    "Python function file functions/f.sql returns must map each column name to its type",
                ),
                ..empty_header()
            },
        },
        FunctionHeaderTestCase {
            description: "a Python scalar UDF with column returns still needs a type string",
            header: header(&[("returns", map(&[("ts", text("FLOAT"))]))]),
            language: FunctionLanguage::Python,
            table_function: false,
            expected_header: FunctionHeader {
                failure: failure(
                    HeaderStage::Start,
                    "Python function file functions/f.sql must declare returns",
                ),
                ..empty_header()
            },
        },
    ];

    for test_case in test_cases {
        assert_eq!(
            parse_function_header(
                &test_case.header,
                test_case.language,
                test_case.table_function,
                "functions/f.sql"
            ),
            test_case.expected_header,
            "{}",
            test_case.description
        );
    }
}
