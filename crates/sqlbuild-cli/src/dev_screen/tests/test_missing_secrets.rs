//! Providers missing secrets show a warning row naming them and their variables until a health read shows none.

use crate::dev_screen::tests::helpers::{play, secrets_reported, still_running};
use crate::dev_screen::tests::test_types::MissingSecretsTestCase;

const TALL: Option<(usize, usize)> = Some((80, 24));

#[test]
fn given_providers_missing_secrets_when_health_is_read_then_a_warning_names_them_until_none_is_missing()
 {
    let test_cases = [
        MissingSecretsTestCase {
            description: "a live screen draws the warning above the keys",
            size: TALL,
            reports: vec![vec![("region", vec!["REGION_TOKEN", "REGION_CODE"])]],
            expected_tail: vec![
                "\u{2502} Triggers   41 paused                                                       \u{2502}",
                "\u{2570}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{256f}",
                "\u{26a0} Secrets missing                region REGION_TOKEN, REGION_CODE; set them in",
                "                                 .env or the environment; press r to reload",
                "keys  o open UI \u{b7} l full log \u{b7} r recompile \u{b7} q quit",
            ],
        },
        MissingSecretsTestCase {
            description: "a live screen drops the warning once a read shows none",
            size: TALL,
            reports: vec![
                vec![("region", vec!["REGION_TOKEN", "REGION_CODE"])],
                vec![],
            ],
            expected_tail: vec![
                "\u{2502} Triggers   41 paused                                                       \u{2502}",
                "\u{2570}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{256f}",
                "keys  o open UI \u{b7} l full log \u{b7} r recompile \u{b7} q quit",
            ],
        },
        MissingSecretsTestCase {
            description: "plain output prints the warning once per change, then that the secrets are set",
            size: None,
            reports: vec![
                vec![("region", vec!["REGION_TOKEN"])],
                vec![("region", vec!["REGION_TOKEN"])],
                vec![],
                vec![],
            ],
            expected_tail: vec![
                "  triggers   41 paused",
                "\u{26a0} Secrets missing                region REGION_TOKEN; set them in .env or the",
                "                                 environment; press r to reload",
                "\u{2713} Secrets set                    no provider is missing a secret",
            ],
        },
    ];

    for test_case in test_cases {
        let viewport: Vec<String> = play(
            test_case.size,
            secrets_reported(test_case.reports),
            still_running,
        )
        .viewport;
        let tail: &[String] =
            &viewport[viewport.len().saturating_sub(test_case.expected_tail.len())..];
        assert_eq!(
            tail,
            test_case.expected_tail.as_slice(),
            "{}",
            test_case.description
        );
    }
}
