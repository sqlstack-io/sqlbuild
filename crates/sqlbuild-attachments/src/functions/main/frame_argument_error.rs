//! A polars table function argument whose type Snowflake passes as Python `Decimal` objects.

use crate::functions::constants::{FIXED_POINT_TYPES, POLARS_DATAFRAME};

/// The first argument of a polars frame whose type is a fixed-point number with a scale above zero.
#[must_use]
pub fn frame_argument_error(
    label: &str,
    dataframe: Option<&str>,
    arguments: &[(String, String)],
) -> Option<String> {
    if dataframe != Some(POLARS_DATAFRAME) {
        return None;
    }
    arguments
        .iter()
        .find(|(_, type_text)| fractional_fixed_point(type_text))
        .map(|(name, type_text)| {
            format!(
                "{label} argument '{name}' is {type_text}, which Snowflake passes to a polars \
                 frame as Python Decimal objects, a slow object column; declare it as FLOAT, or \
                 as INTEGER when it has no fraction"
            )
        })
}

/// Whether `type_text` is `NUMBER`, `DECIMAL`, `DEC` or `NUMERIC` `(p, s)` with `s` above zero.
fn fractional_fixed_point(type_text: &str) -> bool {
    let Some((name, parameters)) = type_text.split_once('(') else {
        return false;
    };
    if !FIXED_POINT_TYPES.contains(&name.trim().to_ascii_uppercase().as_str()) {
        return false;
    }
    parameters
        .trim_end()
        .trim_end_matches(')')
        .split(',')
        .nth(1)
        .is_some_and(|scale| positive_integer(scale.trim()))
}

/// Whether `text` is a run of ASCII digits that is not all zeros.
fn positive_integer(text: &str) -> bool {
    !text.is_empty()
        && text.bytes().all(|byte| byte.is_ascii_digit())
        && text.bytes().any(|byte| byte != b'0')
}
