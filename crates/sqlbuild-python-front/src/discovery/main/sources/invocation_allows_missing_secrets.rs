//! Whether the running invocation passed `--allow-missing-secrets`.

use crate::discovery::_helpers::sources::provider_secrets::INVOCATION_ALLOWS_MISSING_SECRETS;

/// Whether the running invocation passed `--allow-missing-secrets`.
pub fn invocation_allows_missing_secrets() -> bool {
    INVOCATION_ALLOWS_MISSING_SECRETS.get()
}
