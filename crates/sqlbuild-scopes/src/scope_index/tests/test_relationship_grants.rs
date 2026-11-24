use crate::scope_index::tests::helpers::grant_texts;
use crate::scope_index::tests::test_types::{DeclarationRow, GrantTestCase, ResourceRow};

const RESOURCES: &[ResourceRow] = &[
    ("model", "orders", "sql/models/sales/orders.sql"),
    ("model", "other", "sql/models/other/other.sql"),
    ("test", "t1", "tests/unit/t1.sql"),
    ("test", "t2", "tests/unit/t2.sql"),
];
const DECLARATIONS: &[DeclarationRow] = &[
    (
        "constant",
        "rate",
        "sql/models/sales/_sqlbuild/constants/r.sql",
        1,
        "inherited",
        Some("sql/models/sales"),
    ),
    (
        "macro",
        "fmt",
        "sql/models/sales/_macros/m.py",
        3,
        "local",
        Some("sql/models/sales"),
    ),
    (
        "macro",
        "unused",
        "sql/models/sales/_macros/m.py",
        9,
        "local",
        Some("sql/models/sales"),
    ),
    ("macro", "g", "sql/macros/g.py", 1, "global", None),
    (
        "constant",
        "_own",
        "sql/models/sales/orders.sql",
        1,
        "private",
        Some("sql/models/sales"),
    ),
];

#[test]
fn given_relationship_facts_when_granting_then_grants_match_python() {
    let test_cases = [
        GrantTestCase {
            description: "expected models grant visible non-private declarations; macros if called",
            resources: RESOURCES,
            declarations: DECLARATIONS,
            facts: &[("t1", &["orders", "missing", "orders"], &["fmt"], &[])],
            expected_grants: &[
                "test:t1 constant:rate model:orders expected_model",
                "test:t1 macro:fmt model:orders expected_model",
            ],
        },
        GrantTestCase {
            description: "tested macros grant their folder's declarations the test cannot see",
            resources: RESOURCES,
            declarations: DECLARATIONS,
            facts: &[("t2", &[], &[], &["fmt", "unknown"])],
            expected_grants: &[
                "test:t2 constant:rate macro:fmt tested_macro",
                "test:t2 macro:fmt macro:fmt tested_macro",
                "test:t2 macro:unused macro:fmt tested_macro",
            ],
        },
        GrantTestCase {
            description: "a model without scoped neighbours grants only called global macros",
            resources: RESOURCES,
            declarations: DECLARATIONS,
            facts: &[("t1", &["other"], &["g"], &[])],
            expected_grants: &["test:t1 macro:g model:other expected_model"],
        },
    ];

    for test_case in test_cases {
        assert_eq!(
            grant_texts(&test_case),
            test_case.expected_grants,
            "{}",
            test_case.description
        );
    }
}
