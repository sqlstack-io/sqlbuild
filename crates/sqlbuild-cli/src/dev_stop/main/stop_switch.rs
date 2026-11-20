//! The process's one stop switch.

use std::sync::OnceLock;

use crate::dev_stop::models::StopSwitch;

static SWITCH: OnceLock<StopSwitch> = OnceLock::new();

/// The switch the signal task, the key thread and every `sqb dev` role share.
pub(crate) fn stop_switch() -> &'static StopSwitch {
    SWITCH.get_or_init(StopSwitch::new)
}
