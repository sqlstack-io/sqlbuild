//! Whether a dynamic model's listed refresh settings differ from its declared ones.

use crate::dynamic_refresh::_helpers::target_lag::parse_target_lag;
use crate::dynamic_refresh::models::{DynamicSettings, LiveDynamicTable};

/// True when the lag differs or cannot be read, or a declared warehouse differs.
pub fn dynamic_settings_drift(desired: &DynamicSettings, live: &LiveDynamicTable) -> bool {
    if parse_target_lag(&live.target_lag) != Some(desired.lag) {
        return true;
    }
    desired
        .warehouse
        .as_deref()
        .is_some_and(|warehouse| !warehouse.eq_ignore_ascii_case(live.warehouse.trim()))
}
