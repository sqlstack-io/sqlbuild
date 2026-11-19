//! A dynamic table's listed `target_lag`, such as `DOWNSTREAM` or `2 hours`.

use crate::dynamic_refresh::constants::{DOWNSTREAM_TARGET_LAG, TARGET_LAG_UNITS};
use crate::dynamic_refresh::models::RefreshLag;

/// The lag `text` lists, or `None` when it is not a lag Snowflake lists.
pub(crate) fn parse_target_lag(text: &str) -> Option<RefreshLag> {
    let text: &str = text.trim();
    if text.eq_ignore_ascii_case(DOWNSTREAM_TARGET_LAG) {
        return Some(RefreshLag::Downstream);
    }
    let (amount, unit) = text.split_once(' ')?;
    let amount: u64 = match amount.parse::<u64>() {
        Ok(amount) => amount,
        Err(_) => return None,
    };
    let unit: String = unit.trim().to_ascii_lowercase();
    let unit: &str = unit.strip_suffix('s').unwrap_or(&unit);
    TARGET_LAG_UNITS
        .iter()
        .find(|(name, _)| *name == unit)
        .and_then(|(_, seconds)| amount.checked_mul(*seconds))
        .map(RefreshLag::Seconds)
}
