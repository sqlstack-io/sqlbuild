//! Compound scheduler return values.

use crate::scheduler::models::{SchedulerCycle, SchedulerLease};
use crate::store::errors::ControlError;
use crate::store::models::ControlStore;

/// Physical process ownership survives leadership loss, but durable writes require a live fence.
pub trait RunLauncher {
    fn poll(
        &mut self,
        store: &ControlStore,
        lease: Option<&SchedulerLease>,
    ) -> impl Future<Output = Result<(), ControlError>> + Send;

    fn shutdown(
        &mut self,
        store: &ControlStore,
        lease: Option<&SchedulerLease>,
    ) -> impl Future<Output = Result<(), ControlError>> + Send;

    /// Hears each completed cycle, with its phase timings, before the launcher polls.
    fn cycled(&mut self, _cycle: &SchedulerCycle) {}

    /// Hears each transient store failure the scheduler retries instead of stopping.
    fn retrying(&mut self, _error: &ControlError) {}
}
