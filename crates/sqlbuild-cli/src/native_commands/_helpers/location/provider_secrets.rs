//! Reporting the providers a publication shows missing secrets.

use std::collections::BTreeMap;

use sqlbuild_core::process_environment::main::env_file_shown::env_file_shown;
use sqlbuild_core::process_environment::main::location_secret_fix::location_secret_fix;
use sqlbuild_orchestration::dev_events::main::emit_dev_event::emit_dev_event;
use sqlbuild_orchestration::dev_events::models::{DevEvent, UnsetProviderSecrets};
use sqlbuild_orchestration::platform::main::health::unallowed_secret_count::unallowed_secret_count;
use sqlbuild_orchestration::platform::models::{LocationHealth, MissingProviderSecrets};
use uuid::Uuid;

use crate::argv::models::Stream;
use crate::native_commands::_helpers::command::terminal::write_output;
use crate::native_commands::_helpers::location::execution::report_location_health;
use crate::native_commands::models::LocationServe;
use crate::native_commands::types::GenerationSecrets;

/// The providers whose missing secrets stop runs, with what needs each: a provider the target allows to miss secrets is left out.
pub(crate) fn unset_provider_secrets(
    providers: &[MissingProviderSecrets],
    needed_by: &BTreeMap<String, Vec<String>>,
) -> Vec<UnsetProviderSecrets> {
    let mut unset: Vec<UnsetProviderSecrets> = Vec::new();
    for provider in providers.iter().filter(|provider| !provider.allowed) {
        unset.push(UnsetProviderSecrets {
            provider: provider.provider.clone(),
            path: provider.path.clone(),
            settings: provider.settings.clone(),
            needed_by: needed_by
                .get(&provider.provider)
                .cloned()
                .unwrap_or_default(),
        });
    }
    unset
}

/// Report the providers a publication shows missing secrets; under `sqb dev`, also tell the screen, which warns until a report shows none.
pub(crate) async fn report_provider_secrets(
    options: &LocationServe,
    (bearer, session_id): (&str, Uuid),
    (generation, (providers, needed_by)): (i64, GenerationSecrets),
) {
    let env_file: Option<String> = env_file_shown();
    let missing: u64 = unallowed_secret_count(&providers);
    if missing > 0 {
        let mut variables: Vec<String> = Vec::new();
        for provider in providers.iter().filter(|provider| !provider.allowed) {
            for setting in &provider.settings {
                variables.push(format!("{} {}", provider.provider, setting.variable));
            }
        }
        drop(write_output(&[(
            Stream::Stderr,
            &format!(
                "Providers are missing secrets in this location's environment; runs that use them stop before the warehouse: {}\nTo fix, {}.\n",
                variables.join(", "),
                location_secret_fix("each variable", env_file.as_deref())
            ),
        )]));
    }
    drop(emit_dev_event(&DevEvent::MissingSecrets {
        providers: unset_provider_secrets(&providers, &needed_by),
        env_file,
    }));
    report_location_health(
        options,
        bearer,
        session_id,
        LocationHealth::ProviderSecrets {
            generation,
            missing,
            providers,
        },
    )
    .await;
}
