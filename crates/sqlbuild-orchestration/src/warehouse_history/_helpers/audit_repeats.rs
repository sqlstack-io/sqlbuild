//! A run read a page repeats while it follows a run is audited once per window, not on every poll.

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex, PoisonError};
use std::time::{Duration, Instant};

use uuid::Uuid;

/// When each person last had each kind of read of each run audited, on this server.
static AUDITED: LazyLock<Mutex<HashMap<(String, String, Uuid), Instant>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Whether a read by `actor` of kind `action` of `run_id` needs a new audit row: none within `window`.
pub(crate) fn audit_due((actor, action, run_id): (&str, &str, Uuid), window: Duration) -> bool {
    let mut audited = AUDITED.lock().unwrap_or_else(PoisonError::into_inner);
    let now = Instant::now();
    audited.retain(|_, at| now.duration_since(*at) < window);
    !audited.contains_key(&(actor.to_owned(), action.to_owned(), run_id))
}

/// Remember that such a read was audited now.
pub(crate) fn record_audit((actor, action, run_id): (&str, &str, Uuid)) {
    AUDITED
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .insert((actor.to_owned(), action.to_owned(), run_id), Instant::now());
}
