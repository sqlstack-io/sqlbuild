//! The secrets that make a location's provider check a health problem.

use crate::platform::models::MissingProviderSecrets;

/// How many unset secrets belong to providers the target doesn't allow to miss them.
#[must_use]
pub fn unallowed_secret_count(providers: &[MissingProviderSecrets]) -> u64 {
    providers
        .iter()
        .filter(|provider| !provider.allowed)
        .map(|provider| provider.settings.len() as u64)
        .sum()
}
