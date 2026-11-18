//! Public constants of native threads.

/// Marks a thread whose writes command-output capture ignores, such as one running a command-output sink.
pub const OUTPUT_SUPPRESSED_ATTRIBUTE: &str = "_sqlbuild_command_output_suppressed";

/// The compile pool's fixed worker count.
pub const PREPASS_WORKERS: usize = 4;
/// Each compile pool worker's stack size.
pub const WORKER_STACK_BYTES: usize = 16 * 1024 * 1024;
