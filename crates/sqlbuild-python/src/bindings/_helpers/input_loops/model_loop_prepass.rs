//! The model loop's compile pool off the GIL: header facts first, then work the loop hands off.

use std::sync::{Arc, OnceLock};

use rayon::prelude::{IntoParallelRefIterator, ParallelIterator};
use rayon::{ThreadPool, ThreadPoolBuildError, ThreadPoolBuilder};
use sqlbuild_discovery::model_files::main::header_facts::header_facts;
use sqlbuild_discovery::model_files::models::HeaderFacts;
use sqlbuild_sqltext::compiler::models::AuthoredValue;

use crate::bindings::_helpers::input_loops::model_loop_host::ModelLoopRow;

/// The compile pools' fixed worker count.
const PREPASS_WORKERS: usize = 4;
const WORKER_STACK_BYTES: usize = 16 * 1024 * 1024;

static POOL: OnceLock<Result<ThreadPool, ThreadPoolBuildError>> = OnceLock::new();

/// A model's native header values and the relative path parse messages name.
pub(crate) type NativeHeader = (Arc<[(String, AuthoredValue)]>, String);

/// Each model's header facts on the compile pool, or in turn when the pool cannot start.
pub(crate) fn all_header_facts(headers: &[Option<NativeHeader>]) -> Vec<Option<HeaderFacts>> {
    pooled(headers, facts_of)
}

/// `each` of every item in order, run on the compile pool, or in turn when the pool cannot start.
pub(crate) fn pooled<T: Sync, R: Send>(
    items: &[T],
    each: impl Fn(&T) -> R + Sync + Send,
) -> Vec<R> {
    match compile_pool() {
        Ok(pool) => pool.install(|| items.par_iter().map(&each).collect()),
        Err(_) => items.iter().map(each).collect(),
    }
}

/// Run `job` on the compile pool while the caller continues, or now when the pool cannot start.
pub(crate) fn spawned(job: impl FnOnce() + Send + 'static) {
    match compile_pool() {
        Ok(pool) => pool.spawn(job),
        Err(_) => job(),
    }
}

fn compile_pool() -> &'static Result<ThreadPool, ThreadPoolBuildError> {
    POOL.get_or_init(|| {
        ThreadPoolBuilder::new()
            .num_threads(PREPASS_WORKERS)
            .stack_size(WORKER_STACK_BYTES)
            .thread_name(|index| format!("sqlbuild-model-prepass-{index}"))
            .build()
    })
}

fn facts_of(header: &Option<NativeHeader>) -> Option<HeaderFacts> {
    let (values, path) = header.as_ref()?;
    Some(header_facts(values, path))
}

/// The row's native header values and relative path, unless Python constructed its file.
pub(crate) fn native_header(row: &ModelLoopRow) -> Option<NativeHeader> {
    let values: &Arc<[(String, AuthoredValue)]> = row.model_file.get().native_header_values()?;
    Some((Arc::clone(values), row.paths.relative_path.clone()))
}
