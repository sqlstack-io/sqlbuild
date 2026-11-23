use crate::refactor_command::main::refactor_pairs::refactor_pairs;
use crate::refactor_command::models::PairArguments;
use crate::refactor_command::tests::test_types::RefactorPairsTestCase;

#[test]
fn given_words_and_pair_file_when_reading_pairs_then_every_pair_is_read_in_order() {
    let test_cases = [
        RefactorPairsTestCase {
            description: "pairs on the command line, then from the file",
            is_move: true,
            words: &[
                "orders",
                "sql/models/finance/",
                "customers",
                "sql/models/crm/",
            ],
            file: Some(
                "# moves\n\nrefunds\tmodels/finance/\n  totals   sql/models/marts/t.sql  \n",
            ),
            expected_pairs: Ok(&[
                ("orders", "sql/models/finance/"),
                ("customers", "sql/models/crm/"),
                ("refunds", "sql/models/finance/"),
                ("totals", "sql/models/marts/t.sql"),
            ]),
        },
        RefactorPairsTestCase {
            description: "a target without a destination",
            is_move: true,
            words: &["orders", "sql/models/finance/", "customers"],
            file: None,
            expected_pairs: Err("'customers' has no DESTINATION"),
        },
        RefactorPairsTestCase {
            description: "a file line with three words",
            is_move: true,
            words: &[],
            file: Some("orders sql/models/finance/ extra\n"),
            expected_pairs: Err(
                "pairs.tsv:1: expected TARGET and DESTINATION separated by a tab or spaces, \
                 found 3 words",
            ),
        },
        RefactorPairsTestCase {
            description: "nothing to move",
            is_move: true,
            words: &[],
            file: Some("# only a comment\n"),
            expected_pairs: Err("sqb mv needs a TARGET and a DESTINATION"),
        },
        RefactorPairsTestCase {
            description: "renames on the command line, then from the file",
            is_move: false,
            words: &["orders", "orders_v2"],
            file: Some("customers\tclients\n"),
            expected_pairs: Ok(&[("orders", "orders_v2"), ("customers", "clients")]),
        },
        RefactorPairsTestCase {
            description: "a rename target without a new name",
            is_move: false,
            words: &["orders", "orders_v2", "customers"],
            file: None,
            expected_pairs: Err("'customers' has no NEW_NAME"),
        },
        RefactorPairsTestCase {
            description: "a rename file line with one word",
            is_move: false,
            words: &[],
            file: Some("\norders\n"),
            expected_pairs: Err(
                "pairs.tsv:2: expected TARGET and NEW_NAME separated by a tab or spaces, \
                 found 1 words",
            ),
        },
        RefactorPairsTestCase {
            description: "nothing to rename",
            is_move: false,
            words: &[],
            file: None,
            expected_pairs: Err("sqb rename needs a TARGET and a NEW_NAME"),
        },
    ];
    for test_case in test_cases {
        let pairs = refactor_pairs(&PairArguments {
            is_move: test_case.is_move,
            words: test_case.words.to_vec(),
            file: test_case.file.map(|contents| ("pairs.tsv", contents)),
        });

        assert_eq!(
            pairs.map_err(|error| error.message),
            test_case
                .expected_pairs
                .map(|pairs| {
                    pairs
                        .iter()
                        .map(|(target, second)| ((*target).to_owned(), (*second).to_owned()))
                        .collect::<Vec<_>>()
                })
                .map_err(str::to_owned),
            "{}",
            test_case.description
        );
    }
}
