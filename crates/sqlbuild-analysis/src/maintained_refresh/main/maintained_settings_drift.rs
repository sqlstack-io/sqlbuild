//! Whether a maintained model's listed refresh settings differ from its declared ones.

use crate::maintained_refresh::_helpers::target_lag::parse_target_lag;
use crate::maintained_refresh::models::{LiveDynamicTable, MaintainedSettings};

/// True when the lag differs or cannot be read, or a declared warehouse differs.
pub fn maintained_settings_drift(desired: &MaintainedSettings, live: &LiveDynamicTable) -> bool {
    if parse_target_lag(&live.target_lag) != Some(desired.lag) {
        return true;
    }
    desired
        .warehouse
        .as_deref()
        .is_some_and(|warehouse| !warehouse.eq_ignore_ascii_case(live.warehouse.trim()))
}
