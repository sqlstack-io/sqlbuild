//! How a worker retries its last report when the server is briefly unavailable.

use std::time::Duration;

/// Attempts at reporting a refusal before the supervisor's exit reason stands in for it.
pub(crate) const REFUSAL_ATTEMPTS: usize = 3;
/// The wait between attempts.
pub(crate) const REFUSAL_RETRY_DELAY: Duration = Duration::from_millis(200);
/// What a server answers while its store is busy.
pub(crate) const STORE_BUSY_STATUS: u16 = 503;
