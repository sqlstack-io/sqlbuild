//! Limits and audit actions of acknowledgements.

use crate::acknowledgements::models::AcknowledgementEvents;

pub(crate) const MAX_KEY_CHARS: usize = 256;
pub(crate) const MAX_SHAPE_BYTES: usize = 8 * 1024;
pub(crate) const ACKNOWLEDGED: &str = "attention.acknowledged";
pub(crate) const WITHDRAWN: &str = "attention.acknowledgement_withdrawn";
/// Hours after which an acknowledged condition comes back even if it never changes.
pub(crate) const ATTENTION_SNOOZE_HOURS: i64 = 24;
/// Hours of listed condition acknowledgements, so a lapsed one stays readable for a day.
pub(crate) const ATTENTION_LISTING_HOURS: i64 = 48;
/// Days of listed run acknowledgements; the overview lists only a day of problems.
pub(crate) const RUN_LISTING_DAYS: i64 = 7;
pub(crate) const RUN_EVENTS: AcknowledgementEvents = AcknowledgementEvents {
    acknowledged: "run.acknowledged",
    withdrawn: "run.acknowledgement_withdrawn",
    subject: "run_id",
};
pub(crate) const ATTENTION_EVENTS: AcknowledgementEvents = AcknowledgementEvents {
    acknowledged: ACKNOWLEDGED,
    withdrawn: WITHDRAWN,
    subject: "key",
};
