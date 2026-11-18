//! The compile pool shared by model compilation and planning, started on first use.

use std::sync::OnceLock;

use rayon::{ThreadPool, ThreadPoolBuildError, ThreadPoolBuilder};

use crate::native_threads::constants::{PREPASS_WORKERS, WORKER_STACK_BYTES};

static POOL: OnceLock<Result<ThreadPool, ThreadPoolBuildError>> = OnceLock::new();

pub(crate) fn compile_pool() -> &'static Result<ThreadPool, ThreadPoolBuildError> {
    POOL.get_or_init(|| {
        ThreadPoolBuilder::new()
            .num_threads(PREPASS_WORKERS)
            .stack_size(WORKER_STACK_BYTES)
            .thread_name(|index| format!("sqlbuild-model-prepass-{index}"))
            .build()
    })
}
