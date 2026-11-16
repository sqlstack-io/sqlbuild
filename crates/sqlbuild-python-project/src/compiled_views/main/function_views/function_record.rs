//! The function record of an attached function, before assembly resolves its dependencies.

use crate::compiled_views::_helpers::function_views;
use crate::compiled_views::types::AttachedFunction;
use crate::compiled_views::types::FunctionRecord;

/// The function record of an attached function, before assembly resolves its dependencies.
#[inline]
pub fn function_record(attached: AttachedFunction) -> FunctionRecord {
    function_views::function_record(attached)
}
