//! The process's one stop switch.

use crate::dev_stop::_helpers::switch::SWITCH;
use crate::dev_stop::models::StopSwitch;

/// The switch the signal task, the key thread and every `sqb dev` role share.
pub(crate) fn stop_switch() -> &'static StopSwitch {
    SWITCH.get_or_init(StopSwitch::new)
}
