//! A load's work on a thread, and a runtime, whose stacks are a load's.

use std::future::Future;

use crate::runs::constants::LOAD_STACK_BYTES;

/// The future `work` makes, made and driven to completion on a new thread with a load's stack, by a runtime whose threads have it too.
pub(crate) fn on_load_stack<T: Send, F: Future<Output = Result<T, String>>>(
    work: impl FnOnce() -> F + Send,
) -> Result<T, String> {
    std::thread::scope(|scope| {
        std::thread::Builder::new()
            .name("sqlbuild-load".to_owned())
            .stack_size(LOAD_STACK_BYTES)
            .spawn_scoped(scope, || {
                tokio::runtime::Builder::new_multi_thread()
                    .enable_all()
                    .thread_stack_size(LOAD_STACK_BYTES)
                    .build()
                    .map_err(|error| format!("cannot start the load's runtime: {error}"))?
                    .block_on(work())
            })
            .map_err(|error| format!("cannot start the load's thread: {error}"))?
            .join()
            .unwrap_or_else(|panic| std::panic::resume_unwind(panic))
    })
}
