//! Real-store scheduler contracts compiled within the scheduler owner.

#[path = "helpers.rs"]
pub(crate) mod helpers;
#[path = "test_concurrency_source.rs"]
mod test_concurrency_source;
#[path = "test_contract.rs"]
mod test_contract;
#[path = "test_types.rs"]
mod test_types;
#[path = "test_unreadable_schedule_republish.rs"]
mod test_unreadable_schedule_republish;
#[path = "test_unreadable_sensors.rs"]
mod test_unreadable_sensors;
#[path = "ticks/test_ticks.rs"]
pub(crate) mod ticks;
