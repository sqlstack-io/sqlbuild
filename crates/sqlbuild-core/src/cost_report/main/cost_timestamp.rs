//! The UTC instant of a persisted aware cost timestamp.

use crate::cost_report::_helpers::instants::utc_micros;

/// Microseconds since the Unix epoch, or `None` for an invalid or naive timestamp.
pub fn cost_timestamp(value: &str) -> Option<i128> {
    utc_micros(value)
}
