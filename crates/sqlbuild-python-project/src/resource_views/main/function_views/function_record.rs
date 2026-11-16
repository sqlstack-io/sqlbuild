//! The function record of an attached function, before assembly resolves its dependencies.

use crate::resource_views::_helpers::function_views;
use crate::resource_views::types::AttachedFunction;
use crate::resource_views::types::FunctionRecord;

/// The function record of an attached function, before assembly resolves its dependencies.
#[inline]
pub fn function_record(attached: AttachedFunction) -> FunctionRecord {
    function_views::function_record(attached)
}
