//! `run_on_worker_stack`: one call's work on a scoped thread with its own stack size.

/// `work`'s result from a scoped thread with `stack_bytes` of stack, or why the thread failed.
pub fn run_on_worker_stack<T: Send>(
    stack_bytes: usize,
    panic_message: &str,
    work: impl FnOnce() -> T + Send,
) -> Result<T, String> {
    std::thread::scope(|scope| {
        std::thread::Builder::new()
            .stack_size(stack_bytes)
            .spawn_scoped(scope, work)
            .map_err(|error| error.to_string())?
            .join()
            .map_err(|_| panic_message.to_owned())
    })
}
