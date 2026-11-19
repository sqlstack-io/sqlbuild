//! The `TARGET_LAG` value of a refresh lag, in its largest exact unit.

use crate::maintained_refresh::constants::{DOWNSTREAM_TARGET_LAG, TARGET_LAG_UNITS};
use crate::maintained_refresh::models::RefreshLag;

/// `DOWNSTREAM`, or a quoted lag such as `'2 hours'`.
pub fn render_target_lag(lag: RefreshLag) -> String {
    let RefreshLag::Seconds(seconds) = lag else {
        return DOWNSTREAM_TARGET_LAG.to_owned();
    };
    let (unit, size) = TARGET_LAG_UNITS
        .iter()
        .find(|(_, size)| seconds % size == 0)
        .copied()
        .unwrap_or(("second", 1));
    let amount: u64 = seconds / size;
    let plural: &str = if amount == 1 { "" } else { "s" };
    format!("'{amount} {unit}{plural}'")
}
