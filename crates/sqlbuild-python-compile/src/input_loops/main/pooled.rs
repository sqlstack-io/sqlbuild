//! `each` of every item in order, run on the compile pool, or in turn when the pool cannot start.

use crate::input_loops::_helpers::model_loop::model_loop_prepass;

/// `each` of every item in order, run on the compile pool, or in turn when the pool cannot start.
#[inline]
pub fn pooled<T: Sync, R: Send>(items: &[T], each: impl Fn(&T) -> R + Sync + Send) -> Vec<R> {
    model_loop_prepass::pooled(items, each)
}
