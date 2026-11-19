//! Snowflake target lag spellings.

/// The `target_lag` of a dynamic table refreshed for its readers.
pub const DOWNSTREAM_TARGET_LAG: &str = "DOWNSTREAM";
/// Target lag units, largest first, with their seconds.
pub(crate) const TARGET_LAG_UNITS: [(&str, u64); 4] = [
    ("day", 86_400),
    ("hour", 3_600),
    ("minute", 60),
    ("second", 1),
];
