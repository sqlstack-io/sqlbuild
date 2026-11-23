use crate::scope_index::tests::helpers::{resource_groups, visibility_texts};
use crate::scope_index::tests::test_types::LookupTestCase;

#[test]
fn given_identity_keys_when_grouping_then_groups_follow_python_repr_order() {
    let test_cases = [
        LookupTestCase {
            description: "ASCII keys order by repr, where quotes and punctuation sort first",
            resources: &[
                ("model", "ab", "sql/models/ab.sql"),
                ("model", "ab!", "sql/models/ab_bang.sql"),
                ("model", "it's", "sql/models/its.sql"),
                ("function", "zz", "sql/functions/sql/zz.sql"),
                ("seed", "a", "sql/seeds/a.csv"),
            ],
            declarations: &[],
            expected_resource_groups: &[
                "function:zz",
                "model:it's",
                "model:ab!",
                "model:ab",
                "seed:a",
            ],
            expected_visibility: &["global []"],
        },
        LookupTestCase {
            description: "non-ASCII keys order by repr, with escapes for separators",
            resources: &[
                ("model", "caf\u{e9}", "sql/models/cafe_accent.sql"),
                ("model", "cafe", "sql/models/cafe.sql"),
                ("model", "\u{a0}x", "sql/models/nbsp.sql"),
                ("model", "\u{1F600}", "sql/models/emoji.sql"),
                ("model", "Zed", "sql/models/zed.sql"),
                ("model", "e\u{301}", "sql/models/combining.sql"),
            ],
            declarations: &[],
            expected_resource_groups: &[
                "model:Zed",
                "model:\u{a0}x",
                "model:cafe",
                "model:caf\u{e9}",
                "model:e\u{301}",
                "model:\u{1F600}",
            ],
            expected_visibility: &["global []"],
        },
        LookupTestCase {
            description: "visibility positions group by scope over the canonical declarations",
            resources: &[("model", "orders", "sql/models/sales/orders.sql")],
            declarations: &[
                ("macro", "g", "sql/macros/g.py", 1, "global", None),
                (
                    "macro",
                    "l",
                    "sql/models/sales/_macros/l.py",
                    1,
                    "local",
                    Some("sql/models/sales"),
                ),
                (
                    "enum",
                    "i",
                    "sql/models/_sqlbuild/enums/i.sql",
                    1,
                    "inherited",
                    Some("models"),
                ),
                (
                    "constant",
                    "_p",
                    "sql/models/sales/orders.sql",
                    1,
                    "private",
                    Some("sql/models/sales"),
                ),
            ],
            expected_resource_groups: &["model:orders"],
            expected_visibility: &[
                "global [2]",
                "private model:orders [0]",
                "local sql/models/sales [3]",
                "inherited models [1]",
            ],
        },
    ];

    for test_case in test_cases {
        assert_eq!(
            resource_groups(&test_case),
            test_case
                .expected_resource_groups
                .iter()
                .map(|name| (*name).to_owned())
                .collect::<Vec<String>>(),
            "{}",
            test_case.description
        );
        assert_eq!(
            visibility_texts(&test_case),
            test_case.expected_visibility,
            "{}",
            test_case.description
        );
    }
}
