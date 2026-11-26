use crate::loads::main::check_loader::check_loader;
use crate::loads::models::LoaderDeclaration;
use crate::loads::tests::helpers::project_check;
use crate::loads::tests::test_types::CheckLoaderTestCase;

#[test]
fn given_loader_options_when_checking_them_then_sqlload_reads_them_as_a_python_table() {
    let test_cases = [
        CheckLoaderTestCase {
            description: "SQLLoad's table options, with the source's table and columns",
            connection: None,
            settings: "write_strategy = \"merge\"\nunique_key = [\"id\"]\n",
            expected_result: Ok(
                "unique_key = [\"id\"]\nwrite_strategy = \"merge\"\n\n[columns]\nid = \"INTEGER\"\n",
            ),
        },
        CheckLoaderTestCase {
            description: "SQLBuild's old name for replacing the table",
            connection: None,
            settings: "write_strategy = \"table\"\n",
            expected_result: Err("write_strategy \"table\" is now \"replace\""),
        },
        CheckLoaderTestCase {
            description: "an option SQLLoad doesn't know",
            connection: None,
            settings: "cursor_colum = \"id\"\n",
            expected_result: Err("cursor_colum"),
        },
        CheckLoaderTestCase {
            description: "the columns belong to the source",
            connection: None,
            settings: "columns = { id = \"INTEGER\" }\n",
            expected_result: Err(
                "@loader can't set columns; declare each column's type on the source",
            ),
        },
        CheckLoaderTestCase {
            description: "a loader can't read a warehouse connection",
            connection: Some("warehouse"),
            settings: "",
            expected_result: Err(
                "connection 'warehouse' sets no kind; loads and loaders read source connections",
            ),
        },
    ];
    for test_case in test_cases {
        let loader = LoaderDeclaration {
            name: "raw_orders".to_owned(),
            module: "python/nodes/loaders/raw_orders.py".to_owned(),
            connection: test_case.connection.map(str::to_owned),
            settings: toml::from_str(test_case.settings).expect("settings"),
            columns: vec![("id".to_owned(), "INTEGER".to_owned())],
            table: "raw_orders".to_owned(),
        };
        let checked: Result<String, String> = check_loader(&project_check(), &loader)
            .map(|table| toml::to_string(&table).expect("table"))
            .map_err(|problem| problem.message);
        match (checked, test_case.expected_result) {
            (Ok(table), Ok(expected)) => assert_eq!(table, expected, "{}", test_case.description),
            (Err(message), Err(fragment)) => {
                assert!(
                    message.contains(fragment),
                    "{}: {message}",
                    test_case.description
                )
            }
            (checked, _) => panic!("{}: {checked:?}", test_case.description),
        }
    }
}
