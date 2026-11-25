//! The commands that select audits by name and read past runs.

/// The commands whose selectors select audits by name and read past runs.
pub(crate) const RUN_SELECTOR_COMMANDS: [&str; 5] = ["plan", "build", "test", "audit", "check"];
