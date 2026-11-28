//! Limits and audit actions of acknowledgements.

use crate::acknowledgements::models::AcknowledgementEvents;

pub(crate) const MAX_KEY_CHARS: usize = 256;
pub(crate) const MAX_SHAPE_BYTES: usize = 8 * 1024;
pub(crate) const ACKNOWLEDGED: &str = "attention.acknowledged";
pub(crate) const WITHDRAWN: &str = "attention.acknowledgement_withdrawn";
/// A skipped stored event's problem dismissed for good; the skip stays in the history.
pub(crate) const SKIP_DISMISSED: &str = "attention.skipped_event_dismissed";
pub(crate) const MAX_REASON_CHARS: usize = 200;
/// The `attention_snooze` that removes the time limit.
pub(crate) const NO_TIME_LIMIT: &str = "0";
/// Seconds after which an acknowledged condition comes back unchanged, unless `attention_snooze` says otherwise.
pub(crate) const DEFAULT_ATTENTION_SNOOZE_SECONDS: i64 = 86_400;
/// The longest `attention_snooze` with a time limit; `"0"` removes the limit instead.
pub(crate) const MAX_ATTENTION_SNOOZE_DAYS: i64 = 365;
/// Snoozes a listing keeps a condition acknowledgement for, so a lapsed one says why it is back.
pub(crate) const ATTENTION_LISTING_SNOOZES: i32 = 2;
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
