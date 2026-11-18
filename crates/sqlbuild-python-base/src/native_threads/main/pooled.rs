//! Run work over items on the compile pool.

use rayon::prelude::{IntoParallelRefIterator, ParallelIterator};

use crate::native_threads::_helpers::compile_pool::compile_pool;

/// `each` of every item in order, run on the compile pool, or in turn when the pool cannot start.
pub fn pooled<T: Sync, R: Send>(items: &[T], each: impl Fn(&T) -> R + Sync + Send) -> Vec<R> {
    match compile_pool() {
        Ok(pool) => pool.install(|| items.par_iter().map(&each).collect()),
        Err(_) => items.iter().map(each).collect(),
    }
}
