//! Safe display of resolved context values.

use crate::build_command::constants::{
    C0_CONTROL_CODE_LIMIT, C1_CONTROL_CODE_LIMIT, C1_CONTROL_CODE_START, NOT_SET,
};

/// Escape C0 and C1 control characters as `\xNN`.
pub(crate) fn sanitize(value: &str) -> String {
    let mut text: String = String::with_capacity(value.len());
    for character in value.chars() {
        let code: u32 = u32::from(character);
        if code < C0_CONTROL_CODE_LIMIT
            || (C1_CONTROL_CODE_START..C1_CONTROL_CODE_LIMIT).contains(&code)
        {
            text.push_str(&format!("\\x{code:02x}"));
        } else {
            text.push(character);
        }
    }
    text
}

/// The sanitized value, or `not set`.
pub(crate) fn display_value(value: Option<&str>) -> String {
    value.map_or_else(|| NOT_SET.to_owned(), sanitize)
}
