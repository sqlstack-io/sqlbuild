use crate::loads::main::loader_source::loader_source;
use crate::loads::models::LoaderDeclaration;
use crate::loads::tests::test_types::LoaderSourceTestCase;

#[test]
fn given_a_loader_when_declaring_its_python_source_then_it_carries_its_table_options() {
    let test_cases = [
        LoaderSourceTestCase {
            description: "a loader's options and the source's columns, under the loader's name",
            connection: None,
            settings: "write_strategy = \"merge\"\nunique_key = [\"id\"]\n",
            expected_source: r#"{"name":"raw_orders","module":"python/nodes/loaders/raw_orders/loader.py","settings":{"table_name":"orders","tables":{"raw_orders":{"columns":{"id":"INTEGER"},"unique_key":["id"],"write_strategy":"merge"}}}}"#,
        },
        LoaderSourceTestCase {
            description: "the source connection it names",
            connection: Some("shop_api"),
            settings: "write_strategy = \"append\"\n",
            expected_source: r#"{"name":"raw_orders","module":"python/nodes/loaders/raw_orders/loader.py","settings":{"connection":"shop_api","table_name":"orders","tables":{"raw_orders":{"columns":{"id":"INTEGER"},"write_strategy":"append"}}}}"#,
        },
    ];
    for test_case in test_cases {
        let loader = LoaderDeclaration {
            name: "raw_orders".to_owned(),
            module: "python/nodes/loaders/raw_orders/loader.py".to_owned(),
            connection: test_case.connection.map(str::to_owned),
            settings: toml::from_str(test_case.settings).expect("settings"),
            columns: vec![("id".to_owned(), "INTEGER".to_owned())],
            table: "orders".to_owned(),
            on_schema_change: None,
            fills_source: true,
        };

        let source = loader_source(&loader).map(|source| source.to_string());

        assert_eq!(
            source,
            Ok(test_case.expected_source.to_owned()),
            "{}",
            test_case.description
        );
    }
}
