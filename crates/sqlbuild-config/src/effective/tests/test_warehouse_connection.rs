use crate::effective::main::warehouse_connection_problem::warehouse_connection_problem;
use crate::effective::models::WarehouseConnection;
use crate::effective::tests::test_types::WarehouseConnectionTestCase;

#[test]
fn given_a_target_connection_when_checking_its_role_then_only_the_adapters_kind_without_http_settings_holds_models()
 {
    let test_cases = [
        WarehouseConnectionTestCase {
            description: "a connection without kind keeps working",
            name: Some("warehouse"),
            kind: None,
            keys: &["account", "user", "kind_of_thing"],
            expected_problem: None,
        },
        WarehouseConnectionTestCase {
            description: "kind may name the project's adapter",
            name: Some("warehouse"),
            kind: Some("snowflake"),
            keys: &["kind", "account"],
            expected_problem: None,
        },
        WarehouseConnectionTestCase {
            description: "a source connection can't hold models",
            name: Some("shop"),
            kind: Some("http"),
            keys: &["kind", "base_url"],
            expected_problem: Some(
                "target 'dev' uses connection 'shop' (http), which can't hold models; this \
                 project's targets need snowflake connections",
            ),
        },
        WarehouseConnectionTestCase {
            description: "an inline or top-level connection is named by its target",
            name: None,
            kind: Some("postgres"),
            keys: &["kind"],
            expected_problem: Some(
                "target 'dev' uses a connection (postgres), which can't hold models; this \
                 project's targets need snowflake connections",
            ),
        },
        WarehouseConnectionTestCase {
            description: "an http setting on a warehouse connection is refused",
            name: Some("warehouse"),
            kind: None,
            keys: &["account", "rate_limit"],
            expected_problem: Some(
                "connections.warehouse: rate_limit is for http connections; a warehouse \
                 connection can't take it",
            ),
        },
        WarehouseConnectionTestCase {
            description: "an http setting on an unnamed connection names the target",
            name: None,
            kind: None,
            keys: &["headers"],
            expected_problem: Some(
                "the connection of target 'dev': headers is for http connections; a warehouse \
                 connection can't take it",
            ),
        },
    ];
    for test_case in test_cases {
        let connection = WarehouseConnection {
            target: "dev",
            name: test_case.name,
            adapter: "snowflake",
            kind: test_case.kind,
            keys: test_case.keys,
        };
        assert_eq!(
            warehouse_connection_problem(&connection).as_deref(),
            test_case.expected_problem,
            "{}",
            test_case.description
        );
    }
}
