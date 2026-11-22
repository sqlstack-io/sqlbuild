//! Reporting the providers a publication shows missing secrets.

use sqlbuild_orchestration::dev_events::main::emit_dev_event::emit_dev_event;
use sqlbuild_orchestration::dev_events::models::{DevEvent, UnsetProviderSecrets};
use sqlbuild_orchestration::platform::main::health::unallowed_secret_count::unallowed_secret_count;
use sqlbuild_orchestration::platform::models::{LocationHealth, MissingProviderSecrets};
use uuid::Uuid;

use crate::argv::models::Stream;
use crate::native_commands::_helpers::command::terminal::write_output;
use crate::native_commands::_helpers::location::execution::report_location_health;
use crate::native_commands::models::LocationServe;

/// The providers whose missing secrets stop runs, by the variables they read: a provider the target allows to miss secrets is left out.
pub(crate) fn unset_provider_secrets(
    providers: &[MissingProviderSecrets],
) -> Vec<UnsetProviderSecrets> {
    let mut unset: Vec<UnsetProviderSecrets> = Vec::new();
    for provider in providers.iter().filter(|provider| !provider.allowed) {
        let mut variables: Vec<String> = Vec::new();
        for setting in &provider.settings {
            variables.push(setting.variable.clone());
        }
        unset.push(UnsetProviderSecrets {
            provider: provider.provider.clone(),
            variables,
        });
    }
    unset
}

/// Report the providers a publication shows missing secrets; under `sqb dev`, also tell the screen, which warns until a report shows none.
pub(crate) async fn report_provider_secrets(
    options: &LocationServe,
    (bearer, session_id): (&str, Uuid),
    (generation, providers): (i64, Vec<MissingProviderSecrets>),
) {
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
                "Providers are missing secrets in this location's environment; runs that use them stop before the warehouse: {}\n",
                variables.join(", ")
            ),
        )]));
    }
    drop(emit_dev_event(&DevEvent::MissingSecrets {
        providers: unset_provider_secrets(&providers),
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
