//! Exact numeric ordering for persisted decimal metrics.

use crate::cost_report::_helpers::decimal_text::sort_key;
use crate::cost_report::models::DecimalSortKey;

/// Return the numeric key of a finite cost decimal, without floating-point rounding.
pub fn cost_decimal_key(value: &str) -> Option<DecimalSortKey> {
    sort_key(value)
}
