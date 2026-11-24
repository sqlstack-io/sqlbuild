use crate::graph::main::named_selector_atoms::named_selector_atoms;
use crate::graph::tests::test_types::NamedAtomsTestCase;

#[test]
fn given_a_selection_when_listing_named_atoms_then_only_single_node_names_count() {
    let test_cases = [
        NamedAtomsTestCase {
            description: "bare names count",
            selection: "refund_review fct_orders",
            expected_atoms: &["refund_review", "fct_orders"],
        },
        NamedAtomsTestCase {
            description: "a bare name inside an intersection counts, its tag does not",
            selection: "fct_orders,tag:orders",
            expected_atoms: &["fct_orders"],
        },
        NamedAtomsTestCase {
            description: "typed names count",
            selection: "asset:refund_review source:raw_orders seed:countries test:orders_unique",
            expected_atoms: &[
                "asset:refund_review",
                "source:raw_orders",
                "seed:countries",
                "test:orders_unique",
            ],
        },
        NamedAtomsTestCase {
            description: "tags do not count",
            selection: "tag:orders tag:finance",
            expected_atoms: &[],
        },
        NamedAtomsTestCase {
            description: "kinds do not count",
            selection: "kind:model kind:asset",
            expected_atoms: &[],
        },
        NamedAtomsTestCase {
            description: "folder paths do not count",
            selection: "sql/models/orders python/nodes/orders path:sql/models/finance",
            expected_atoms: &[],
        },
        NamedAtomsTestCase {
            description: "graph expansions and path selectors do not count",
            selection: "+fct_orders fct_orders+ +fct_refunds+ stg_orders~fct_orders",
            expected_atoms: &[],
        },
        NamedAtomsTestCase {
            description: "globs do not count",
            selection: "fct_orders_* asset:refund_? fct_[ab]",
            expected_atoms: &[],
        },
        NamedAtomsTestCase {
            description: "job selectors do not count",
            selection: "job:orders_nightly_job",
            expected_atoms: &[],
        },
        NamedAtomsTestCase {
            description: "extra whitespace adds nothing",
            selection: "  fct_orders \n\t fct_refunds  ",
            expected_atoms: &["fct_orders", "fct_refunds"],
        },
    ];
    for test_case in test_cases {
        assert_eq!(
            named_selector_atoms(test_case.selection),
            test_case.expected_atoms,
            "{}",
            test_case.description
        );
    }
}
