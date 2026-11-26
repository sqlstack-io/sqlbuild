use crate::source_connections::main::source_connections::source_connections;
use crate::source_connections::tests::test_types::SourceConnectionsTestCase;

#[test]
fn given_source_connection_settings_when_reading_them_then_sqlload_checks_them_and_templates_stay_text()
 {
    let test_cases = [
        SourceConnectionsTestCase {
            description: "an http connection with a templated token and a rate limit",
            toml: "[shop]\nkind = \"http\"\nbase_url = \"https://shop.example\"\n\
                   auth = { kind = \"bearer\" }\ntoken = \"${env:SHOP_TOKEN}\"\nrate_limit = \"10/s\"\n\
                   [files]\nkind = \"s3\"\nbucket = \"landing\"\n",
            expected: Ok(&[("files", "s3"), ("shop", "http")]),
        },
        SourceConnectionsTestCase {
            description: "an unknown setting gets SQLLoad's message",
            toml: "[shop]\nkind = \"http\"\nbase_url = \"https://shop.example\"\nrate_limt = \"10/s\"\n",
            expected: Err(
                "unknown field `rate_limt`, expected one of `base_url`, `auth`, `token`, `username`, \
                 `password`, `client_id`, `client_secret`, `refresh_token`, `headers`, \
                 `rate_limit`, `max_in_flight`, `retries`\nin `connections.shop`\n",
            ),
        },
        SourceConnectionsTestCase {
            description: "an unknown kind lists SQLLoad's kinds",
            toml: "[shop]\nkind = \"htp\"\n",
            expected: Err(
                "unknown variant `htp`, expected one of `snowflake`, `databricks`, `bigquery`, \
                 `clickhouse`, `postgres`, `s3`, `gcs`, `filesystem`, `sftp`, `duckdb`, \
                 `motherduck`, `odbc`, `google_sheets`, `http`\nin `connections.shop.kind`\n",
            ),
        },
        SourceConnectionsTestCase {
            description: "a template where SQLLoad wants a number fails as the text the user wrote",
            toml: "[erp]\nkind = \"postgres\"\nhost = \"db\"\nuser = \"reader\"\ndatabase = \"erp\"\n\
                   port = \"${env:ERP_PORT}\"\n",
            expected: Err(
                "invalid type: string \"${env:ERP_PORT}\", expected u16\nin `connections.erp`\n",
            ),
        },
    ];
    for test_case in test_cases {
        let table: toml::Table = toml::from_str(test_case.toml).expect("test TOML");
        let read = source_connections(table).map(|connections| {
            connections
                .iter()
                .map(|(name, connection)| (name.clone(), connection.kind()))
                .collect::<Vec<_>>()
        });
        let expected = test_case.expected.map(|pairs| {
            pairs
                .iter()
                .map(|(name, kind)| ((*name).to_owned(), *kind))
                .collect::<Vec<_>>()
        });
        assert_eq!(
            read.as_ref().map_err(String::as_str),
            expected.as_ref().map_err(|message| *message),
            "{}",
            test_case.description
        );
    }
}
