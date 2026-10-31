//! Compound scheduler return values.

use chrono::{DateTime, Utc};

use crate::scheduler::models::SchedulerLease;
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
}

/// Expanded wall-clock times and the first omitted time, if the batch was bounded.
pub(crate) type ScheduleExpansion = (Vec<DateTime<Utc>>, Option<DateTime<Utc>>);
