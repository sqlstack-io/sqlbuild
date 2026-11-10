use crate::assembly::analysis_session::tests::helpers::sharing_lines;
use crate::assembly::analysis_session::tests::test_types::NativeSharingTestCase;

#[test]
fn given_models_of_one_wave_when_analysing_then_only_models_with_equal_keys_share() {
    let test_cases = [
        NativeSharingTestCase {
            description: "only the model whose relation has the same shape shares, keeping its own names",
            relations: &[
                ("src_a", &[("id", "INTEGER"), ("amount", "DOUBLE")]),
                ("src_b", &[("id", "INTEGER"), ("amount", "DOUBLE")]),
                ("src_c", &[("id", "VARCHAR"), ("amount", "DOUBLE")]),
            ],
            models: &[
                (
                    "from_a",
                    "SELECT id, amount + 1 AS bumped FROM __source(\"src_a\")",
                    &["src_a"],
                ),
                (
                    "from_b",
                    "SELECT id, amount + 1 AS bumped FROM __source(\"src_b\")",
                    &["src_b"],
                ),
                (
                    "from_c",
                    "SELECT id, amount + 1 AS bumped FROM __source(\"src_c\")",
                    &["src_c"],
                ),
            ],
            expected_lines: &[
                &[
                    "id INTEGER",
                    "bumped DOUBLE",
                    "id <- source:src_a.id",
                    "bumped <- source:src_a.amount",
                ],
                &[
                    "id INTEGER",
                    "bumped DOUBLE",
                    "id <- source:src_b.id",
                    "bumped <- source:src_b.amount",
                ],
                &[
                    "id VARCHAR",
                    "bumped DOUBLE",
                    "id <- source:src_c.id",
                    "bumped <- source:src_c.amount",
                ],
            ],
            expected_shares: [
                ("native_analysis_shared", 1),
                ("native_analysis_unshared", 2),
            ],
        },
        NativeSharingTestCase {
            description: "only the model whose reference names sort like the first one's shares, since lineage lists sources by name",
            relations: &[
                ("src_a", &[("id", "INTEGER")]),
                ("src_b", &[("id", "INTEGER")]),
                ("src_c", &[("id", "INTEGER")]),
                ("src_d", &[("id", "INTEGER")]),
                ("src_e", &[("id", "INTEGER")]),
                ("src_f", &[("id", "INTEGER")]),
            ],
            models: &[
                (
                    "ab",
                    "SELECT l.id + r.id AS total FROM __source(\"src_a\") AS l JOIN __source(\"src_b\") AS r ON l.id = r.id",
                    &["src_a", "src_b"],
                ),
                (
                    "cd",
                    "SELECT l.id + r.id AS total FROM __source(\"src_c\") AS l JOIN __source(\"src_d\") AS r ON l.id = r.id",
                    &["src_c", "src_d"],
                ),
                (
                    "fe",
                    "SELECT l.id + r.id AS total FROM __source(\"src_f\") AS l JOIN __source(\"src_e\") AS r ON l.id = r.id",
                    &["src_f", "src_e"],
                ),
            ],
            expected_lines: &[
                &["total INTEGER", "total <- source:src_a.id source:src_b.id"],
                &["total INTEGER", "total <- source:src_c.id source:src_d.id"],
                &["total INTEGER", "total <- source:src_e.id source:src_f.id"],
            ],
            expected_shares: [
                ("native_analysis_shared", 1),
                ("native_analysis_unshared", 2),
            ],
        },
        NativeSharingTestCase {
            description: "a reference glued into a longer name never shares: string_split and events_split name different functions",
            relations: &[
                ("string", &[("note", "VARCHAR")]),
                ("events", &[("note", "VARCHAR")]),
            ],
            models: &[
                (
                    "from_string",
                    "SELECT __source(\"string\")_split(note, ',') AS parts FROM __source(\"string\")",
                    &["string"],
                ),
                (
                    "from_events",
                    "SELECT __source(\"events\")_split(note, ',') AS parts FROM __source(\"events\")",
                    &["events"],
                ),
            ],
            expected_lines: &[
                &["parts -", "parts <- source:string.note"],
                &["parts -", "parts <- source:events.note"],
            ],
            expected_shares: [
                ("native_analysis_shared", 0),
                ("native_analysis_unshared", 2),
            ],
        },
    ];

    for test_case in test_cases {
        let (lines, shares) = sharing_lines(test_case.relations, test_case.models);

        assert_eq!(lines, test_case.expected_lines, "{}", test_case.description);
        assert_eq!(
            shares, test_case.expected_shares,
            "{}",
            test_case.description
        );
    }
}
