//! Whether the running invocation passed `--allow-missing-secrets`.

use crate::discovery::_helpers::sources::provider_secrets;

/// Whether the running invocation passed `--allow-missing-secrets`.
#[inline]
pub fn invocation_allows_missing_secrets() -> bool {
    provider_secrets::invocation_allows_missing_secrets()
}
