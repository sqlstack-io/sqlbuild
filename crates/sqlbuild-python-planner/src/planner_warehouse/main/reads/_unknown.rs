//! A read whose answer is not known.

use crate::planner_warehouse::models::Read;
use crate::planner_warehouse::types::WarehouseReads;
use std::borrow::Cow;

/// Answer `Unknown(item)` and note the item, so the plan reports it.
pub(in crate::planner_warehouse) fn unknown<T>(
    reads: &dyn WarehouseReads,
    item: impl Into<Cow<'static, str>>,
) -> Read<T> {
    let item: Cow<'static, str> = item.into();
    reads.note_unknown(item.clone().into_owned());
    Read::Unknown(item)
}
