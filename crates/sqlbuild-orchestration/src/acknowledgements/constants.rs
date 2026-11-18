//! Limits of attention acknowledgements.

use chrono::Duration;

/// An acknowledged condition comes back after this long even if it never changes.
pub(crate) fn attention_snooze() -> Duration {
    Duration::hours(24)
}
/// Listed acknowledgements: a lapsed one stays readable for a day, so the Overview can say why the condition is back.
pub(crate) fn attention_listing_window() -> Duration {
    Duration::hours(48)
}
pub(crate) const MAX_KEY_CHARS: usize = 256;
pub(crate) const MAX_SHAPE_BYTES: usize = 8 * 1024;
pub(crate) const ACKNOWLEDGED: &str = "attention.acknowledged";
pub(crate) const WITHDRAWN: &str = "attention.acknowledgement_withdrawn";
