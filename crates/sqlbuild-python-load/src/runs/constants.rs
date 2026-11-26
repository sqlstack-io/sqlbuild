//! What a load's threads need.

/// The stack every thread of a load runs on: a debug build's load goes deeper than Tokio's 2 MiB default.
pub(crate) const LOAD_STACK_BYTES: usize = 8 * 1024 * 1024;
/// How `sqlload plan`'s closing summary line starts.
pub(crate) const PLAN_SUMMARY_MARKS: [&str; 2] = ["\u{2713} Plan", "\u{2717} Plan"];
/// How a section names the `sqlload` command that runs it, right-aligned after two spaces.
pub(crate) const SQLLOAD_COMMAND: &str = "  sqlload ";
