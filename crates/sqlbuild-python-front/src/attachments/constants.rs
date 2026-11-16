//! Public constants of attachments.

/// Executor placeholder replaced with a microbatch's start bound.
pub const MICROBATCH_START_SENTINEL: &str = "__SQB_CURSOR_START__";

/// Executor placeholder replaced with a microbatch's end bound.
pub const MICROBATCH_END_SENTINEL: &str = "__SQB_CURSOR_END__";
