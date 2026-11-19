//! The node setting one plan consumer applies.

use crate::job_settings::constants::JOB_SETTINGS;
use crate::job_settings::models::NodeConsumer;

/// The node setting `consumer` applies, from the table; `None` if no row names it.
#[must_use]
pub fn consumed_setting(consumer: NodeConsumer) -> Option<&'static str> {
    JOB_SETTINGS
        .iter()
        .find(|spec| spec.consumer == Some(consumer))
        .map(|spec| spec.name)
}
