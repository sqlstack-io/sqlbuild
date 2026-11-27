//! A `ctx.log` message as the server stores it.

use crate::run_output::constants::NODE_LOG_MESSAGE_BYTES;

const ELLIPSIS: char = '\u{2026}';

/// `message` without NUL, which PostgreSQL refuses in text and JSON, cut to its limit at a character boundary, ending in an ellipsis when cut.
#[must_use]
pub fn bounded_log_message(message: &str) -> String {
    let message: String = message.replace('\0', "");
    if message.len() <= NODE_LOG_MESSAGE_BYTES {
        return message;
    }
    let room: usize = NODE_LOG_MESSAGE_BYTES - ELLIPSIS.len_utf8();
    let end: usize = (0..=room)
        .rev()
        .find(|index| message.is_char_boundary(*index))
        .unwrap_or(0);
    format!("{}{ELLIPSIS}", &message[..end])
}
