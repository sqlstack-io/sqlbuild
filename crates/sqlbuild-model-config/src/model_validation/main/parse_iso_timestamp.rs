//! The existing CPython-compatible ISO parser for native timestamp consumers.

use crate::model_validation::_helpers::isoformat::{
    datetime_fromisoformat, epoch_micros, local_micros,
};

/// Unix microseconds and timezone awareness, using the selected CPython minor's ISO rules.
pub fn parse_iso_timestamp(text: &str, minor: u8) -> Option<(i64, bool)> {
    let Ok(parsed) = datetime_fromisoformat(text, minor) else {
        return None;
    };
    Some((
        local_micros(&parsed) - epoch_micros() - parsed.offset_micros.unwrap_or(0),
        parsed.offset_micros.is_some(),
    ))
}
