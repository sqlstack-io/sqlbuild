//! The warehouse state a plan reads, indexed once from the facts a builder received.

use crate::warehouse_state::_helpers::indexes::indexed;
use crate::warehouse_state::models::{WarehouseState, WarehouseStateParts};

/// The state over `parts`; lookups after this never scan the facts.
pub fn warehouse_state(parts: WarehouseStateParts) -> WarehouseState {
    indexed(parts)
}
