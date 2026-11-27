use crate::header_strings::models::DescriptionKeys;
use crate::header_strings::tests::helpers::{follows_backwards, in_order, piece_texts, reversed};
use crate::header_strings::tests::test_types::DescriptionKeysTestCase;

#[test]
fn given_any_parenthesis_nesting_when_reading_keys_forward_once_then_meta_matches_the_backward_scan()
 {
    let test_cases = [
        DescriptionKeysTestCase {
            description: "strings asked about in order",
            order: in_order,
            expected_follows: follows_backwards,
        },
        DescriptionKeysTestCase {
            description: "strings asked about in reverse order",
            order: reversed,
            expected_follows: follows_backwards,
        },
    ];
    let pieces: [&str; 9] = [
        "meta (",
        "META(",
        "(",
        ")",
        "description '",
        "x description '",
        "xmeta (",
        " ",
        "a_meta (",
    ];
    let texts: Vec<String> = piece_texts(&pieces, 5);
    for test_case in test_cases {
        let (mut prose, mut not_prose): (usize, usize) = (0, 0);
        for text in &texts {
            let characters: Vec<char> = text.chars().collect();
            let starts: Vec<usize> = (test_case.order)(
                (0..characters.len())
                    .filter(|index| characters[*index] == '\'')
                    .collect(),
            );
            let mut keys: DescriptionKeys<'_, char> = DescriptionKeys::new(&characters);
            for start in starts {
                let follows: bool = keys.follows(start);
                assert_eq!(
                    follows,
                    (test_case.expected_follows)(&characters, start),
                    "{}: {text:?} at {start}",
                    test_case.description
                );
                prose += usize::from(follows);
                not_prose += usize::from(!follows);
            }
        }
        assert!(
            prose > 10_000 && not_prose > 10_000,
            "{}: prose {prose}, not prose {not_prose}",
            test_case.description
        );
    }
}
