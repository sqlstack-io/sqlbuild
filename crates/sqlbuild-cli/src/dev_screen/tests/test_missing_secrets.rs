//! Providers missing secrets show a warning row naming them and their variables until a health read shows none.

use crate::dev_screen::tests::helpers::{play, secrets_reported, still_running};
use crate::dev_screen::tests::test_types::MissingSecretsTestCase;

const TALL: Option<(usize, usize)> = Some((80, 24));

#[test]
fn given_providers_missing_secrets_when_health_is_read_then_a_warning_names_them_until_none_is_missing()
 {
    let test_cases = [
        MissingSecretsTestCase {
            description: "a live screen prints the block and keeps a row above the keys",
            size: TALL,
            env_file: Some("config/dev.env"),
            needed_by: &["source:raw_regions", "model:stg_regions"],
            reports: vec![vec![("region", vec!["REGION_TOKEN", "REGION_CODE"])]],
            expected_tail: vec![
                "\u{26a0} warn   1 provider has missing secrets                          missing secret",
                "  at     python/providers/region.py:9   region_token  \u{2190} REGION_TOKEN",
                "         python/providers/region.py:10  region_code   \u{2190} REGION_CODE",
                "  why    runs that use it stop at the provider check, before the warehouse; 2",
                "         nodes need it (raw_regions, stg_regions)",
                "  fix    set them in config/dev.env and press r, or set them in the environment",
                "         you start sqb dev from, then quit and run sqb dev again",
                "\u{26a0} Secrets missing                region REGION_TOKEN, REGION_CODE",
                "keys  o open UI \u{b7} l full log \u{b7} r recompile \u{b7} q quit",
            ],
        },
        MissingSecretsTestCase {
            description: "without an env file only a restart is offered, and a long node list is folded",
            size: None,
            env_file: None,
            needed_by: &[
                "source:raw_regions",
                "model:a",
                "model:b",
                "model:c",
                "model:d",
                "model:e",
                "model:f",
            ],
            reports: vec![vec![("region", vec!["REGION_TOKEN"])]],
            expected_tail: vec![
                "\u{26a0} warn   1 provider has missing secrets                          missing secret",
                "  at     python/providers/region.py:9  region_token  \u{2190} REGION_TOKEN",
                "  why    runs that use it stop at the provider check, before the warehouse; 7",
                "         nodes need it (raw_regions, a, b, c, d and 2 more)",
                "  fix    set it in the environment you start sqb dev from, then quit and run",
                "         sqb dev again",
            ],
        },
        MissingSecretsTestCase {
            description: "a live screen drops the row once a read shows none",
            size: TALL,
            env_file: Some(".env"),
            needed_by: &[],
            reports: vec![vec![("region", vec!["REGION_TOKEN"])], vec![]],
            expected_tail: vec![
                "\u{2713} Secrets set                    no provider is missing a secret",
                "keys  o open UI \u{b7} l full log \u{b7} r recompile \u{b7} q quit",
            ],
        },
        MissingSecretsTestCase {
            description: "plain output prints the block once per change, then that the secrets are set",
            size: None,
            env_file: Some(".env"),
            needed_by: &[],
            reports: vec![
                vec![("region", vec!["REGION_TOKEN"])],
                vec![("region", vec!["REGION_TOKEN"])],
                vec![],
                vec![],
            ],
            expected_tail: vec![
                "  triggers   41 paused",
                "\u{26a0} warn   1 provider has missing secrets                          missing secret",
                "  at     python/providers/region.py:9  region_token  \u{2190} REGION_TOKEN",
                "  why    runs that use it stop at the provider check, before the warehouse",
                "  fix    set it in .env and press r, or set it in the environment you start sqb",
                "         dev from, then quit and run sqb dev again",
                "\u{2713} Secrets set                    no provider is missing a secret",
            ],
        },
    ];

    for test_case in test_cases {
        let viewport: Vec<String> = play(
            test_case.size,
            secrets_reported(test_case.reports, (test_case.env_file, test_case.needed_by)),
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
