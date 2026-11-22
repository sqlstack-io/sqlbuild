//! `sqb dev` warns only about providers whose missing secrets stop runs, never one the target allows to miss them.

use sqlbuild_orchestration::dev_events::models::UnsetProviderSecrets;
use sqlbuild_orchestration::platform::models::{MissingProviderSecrets, MissingProviderSetting};

use crate::native_commands::_helpers::location::provider_secrets::unset_provider_secrets;
use crate::native_commands::tests::test_types::UnsetProviderSecretsTestCase;

#[test]
fn given_providers_missing_secrets_when_the_dev_warning_is_built_then_allowed_providers_are_left_out()
 {
    let test_cases = [
        UnsetProviderSecretsTestCase {
            description: "a provider the target allows to miss secrets is left out",
            providers: vec![("export_bucket", true, vec!["EXPORT_BUCKET_KEY"])],
            expected_unset: vec![],
        },
        UnsetProviderSecretsTestCase {
            description: "a provider that isn't allowed is kept with every variable it misses",
            providers: vec![("region", false, vec!["REGION_TOKEN", "REGION_CODE"])],
            expected_unset: vec![("region", vec!["REGION_TOKEN", "REGION_CODE"])],
        },
        UnsetProviderSecretsTestCase {
            description: "only the providers that aren't allowed are kept, in order",
            providers: vec![
                ("export_bucket", true, vec!["EXPORT_BUCKET_KEY"]),
                ("region", false, vec!["REGION_TOKEN"]),
                ("payments", false, vec!["PAYMENTS_KEY"]),
            ],
            expected_unset: vec![
                ("region", vec!["REGION_TOKEN"]),
                ("payments", vec!["PAYMENTS_KEY"]),
            ],
        },
    ];

    for test_case in test_cases {
        let providers: Vec<MissingProviderSecrets> = test_case
            .providers
            .iter()
            .map(|(provider, allowed, variables)| MissingProviderSecrets {
                provider: (*provider).to_owned(),
                path: format!("providers/{provider}.py"),
                settings: variables
                    .iter()
                    .map(|variable| MissingProviderSetting {
                        field: variable.to_lowercase(),
                        variable: (*variable).to_owned(),
                        line: None,
                    })
                    .collect(),
                nodes: Vec::new(),
                allowed: *allowed,
            })
            .collect();
        let unset: Vec<(String, Vec<String>)> = unset_provider_secrets(&providers)
            .into_iter()
            .map(
                |UnsetProviderSecrets {
                     provider,
                     variables,
                 }| (provider, variables),
            )
            .collect();

        assert_eq!(
            unset,
            test_case
                .expected_unset
                .iter()
                .map(|(provider, variables)| (
                    (*provider).to_owned(),
                    variables
                        .iter()
                        .map(|variable| (*variable).to_owned())
                        .collect()
                ))
                .collect::<Vec<(String, Vec<String>)>>(),
            "{}",
            test_case.description
        );
    }
}
