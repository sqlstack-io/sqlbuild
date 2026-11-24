use crate::scope_index::main::path_visible::path_visible;
use crate::scope_index::models::{DeclaredScope, ScopeKind};
use crate::scope_index::tests::test_types::PathVisibleTestCase;

#[test]
fn given_declared_scope_when_checking_authored_path_then_python_visibility_is_returned() {
    let test_cases = [
        PathVisibleTestCase {
            description: "global is visible everywhere",
            scope: ScopeKind::Global,
            owning_path: None,
            path: "sql/models/marts/orders.sql",
            expected_visible: true,
        },
        PathVisibleTestCase {
            description: "inherited reaches a descendant folder",
            scope: ScopeKind::Inherited,
            owning_path: Some("sql/models"),
            path: "sql/models/marts/orders.sql",
            expected_visible: true,
        },
        PathVisibleTestCase {
            description: "inherited does not reach a sibling tree",
            scope: ScopeKind::Inherited,
            owning_path: Some("sql/models/staging"),
            path: "sql/models/marts/orders.sql",
            expected_visible: false,
        },
        PathVisibleTestCase {
            description: "local only reaches its own folder",
            scope: ScopeKind::Local,
            owning_path: Some("sql/models"),
            path: "sql/models/marts/orders.sql",
            expected_visible: false,
        },
    ];

    for test_case in test_cases {
        let declared = DeclaredScope {
            scope: test_case.scope,
            owner: None,
            owning_path: test_case.owning_path.map(str::to_owned),
        };
        assert_eq!(
            path_visible(&declared, test_case.path),
            Ok(test_case.expected_visible),
            "{}",
            test_case.description
        );
    }
}
