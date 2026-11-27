//! SQLLoad's run-time messages about a load, naming its settings as the `load:` mapping does.

use crate::loads::constants::{LOAD_KEY, SOURCES};

/// `message` with each `[sources.<load>...]` table SQLLoad names written `[load...]`.
pub fn load_message(load: &str, message: &str) -> String {
    message
        .replace(&format!("[{SOURCES}.{load}]"), &format!("[{LOAD_KEY}]"))
        .replace(&format!("[{SOURCES}.{load}."), &format!("[{LOAD_KEY}."))
}
