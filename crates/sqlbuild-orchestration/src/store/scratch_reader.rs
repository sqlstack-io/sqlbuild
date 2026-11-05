pub const SCRATCH_ORDER_IDS: &str = "SELECT order_id FROM sqb_scratch_orders";

pub enum ScratchState {
    Open,
    Closed,
}

pub fn scratch_state() -> ScratchState {
    ScratchState::Open
}
