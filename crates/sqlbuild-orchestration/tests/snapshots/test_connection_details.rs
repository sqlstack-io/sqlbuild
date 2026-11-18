use serde_json::json;

use crate::snapshots::main::reads::connection_details::connection_details;
use crate::snapshots::tests::test_types::ConnectionDetailsTestCase;

#[test]
fn given_connection_settings_when_details_are_published_then_only_named_non_secret_fields_appear() {
    let test_cases = [
        ConnectionDetailsTestCase {
            description: "Snowflake key pair names the account, user, role, warehouse and database",
            adapter: "snowflake",
            settings: json!({"account": "orders-account", "user": "orders_loader", "role": "LOADER",
                "warehouse": "ORDERS_WH", "database": "ORDERS", "schema": "PUBLIC",
                "private_key_path": "/keys/orders.p8", "private_key_file_pwd": "hunter2"}),
            expected_details: json!({"adapter": "snowflake", "account": "orders-account", "host": null,
                "user": "orders_loader", "role": "LOADER", "warehouse": "ORDERS_WH",
                "database": "ORDERS", "auth_method": "key_pair"}),
        },
        ConnectionDetailsTestCase {
            description: "a Snowflake authenticator names the method; unknown ones are kept as written",
            adapter: "snowflake",
            settings: json!({"account": "orders-account", "authenticator": "ExternalBrowser"}),
            expected_details: json!({"adapter": "snowflake", "account": "orders-account", "host": null,
                "user": null, "role": null, "warehouse": null, "database": null,
                "auth_method": "external_browser"}),
        },
        ConnectionDetailsTestCase {
            description: "Postgres names its host, user and dbname; the password is only a method",
            adapter: "postgres",
            settings: json!({"host": "db.example.com", "user": "inventory", "dbname": "inventory",
                "password": "hunter2", "port": "5432"}),
            expected_details: json!({"adapter": "postgres", "account": null, "host": "db.example.com",
                "user": "inventory", "role": null, "warehouse": null, "database": "inventory",
                "auth_method": "password"}),
        },
        ConnectionDetailsTestCase {
            description: "a DuckDB file is the host, with no database or sign-in",
            adapter: "duckdb",
            settings: json!({"database": "warehouse/customers.duckdb"}),
            expected_details: json!({"adapter": "duckdb", "account": null, "host": "warehouse/customers.duckdb",
                "user": null, "role": null, "warehouse": null, "database": null,
                "auth_method": "none"}),
        },
        ConnectionDetailsTestCase {
            description: "a token-only connection names the method and no value",
            adapter: "motherduck",
            settings: json!({"database": "md:customers", "token": "hunter2"}),
            expected_details: json!({"adapter": "motherduck", "account": null, "host": null,
                "user": null, "role": null, "warehouse": null, "database": "md:customers",
                "auth_method": "token"}),
        },
    ];
    for test_case in test_cases {
        let settings = test_case.settings.as_object().expect("settings").clone();
        let published = json!(connection_details(test_case.adapter, &settings));
        assert_eq!(
            published, test_case.expected_details,
            "{}",
            test_case.description
        );
        assert!(
            !published.to_string().contains("hunter2"),
            "{}: never a secret",
            test_case.description
        );
    }
}
