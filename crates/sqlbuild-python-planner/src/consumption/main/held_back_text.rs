//! Why `--changed` holds a node back, as one line for verbose output.

use sqlbuild_state::consumption::models::HeldBack;

/// Why `--changed` holds a node back, as one line for verbose output.
pub fn held_back_text(why: &HeldBack) -> String {
    match why {
        HeldBack::RefreshedBy(jobs) => format!(
            "job {} refreshes it, and the change is not in that job",
            jobs.join(", ")
        ),
        HeldBack::ReadsHeldBack(key) => format!("reads held-back {} {}", key.kind, key.name),
    }
}
