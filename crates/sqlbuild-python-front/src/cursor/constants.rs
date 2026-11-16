//! Public constants of cursor.

use sqlbuild_cursor::algebra::models::Grain;

/// Cursor grain names, finest first.
pub const GRAIN_NAMES: [(&str, Grain); 6] = [
    ("second", Grain::Second),
    ("minute", Grain::Minute),
    ("hour", Grain::Hour),
    ("day", Grain::Day),
    ("month", Grain::Month),
    ("year", Grain::Year),
];

pub const DATETIME_MODULE: &str = "datetime";
