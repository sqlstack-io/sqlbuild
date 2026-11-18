//! Hand a job to the compile pool.

use crate::native_threads::_helpers::compile_pool::compile_pool;

/// Run `job` on the compile pool while the caller continues, or now when the pool cannot start.
pub fn spawned(job: impl FnOnce() + Send + 'static) {
    match compile_pool() {
        Ok(pool) => pool.spawn(job),
        Err(_) => job(),
    }
}
