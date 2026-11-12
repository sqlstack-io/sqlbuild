//! The expression-source columns a build's planning described, kept for its observations.

use std::cell::RefCell;

use serde_json::{Map, Value};
use sqlbuild_cache::digest::main::sha256_hex::sha256_hex;
use sqlbuild_state::observations::constants::{
    SOURCE_COLUMNS_NAMES_KEY, SOURCE_COLUMNS_SQL_HASH_KEY,
};

/// Each described source's name and observation value, in the order planning described them.
pub(crate) type DescribedSources = Vec<(String, String)>;

thread_local! {
    /// Per thread, since a build plans and records on the thread that runs it; `None` when not armed.
    static CAPTURED: RefCell<Option<DescribedSources>> = const { RefCell::new(None) };
}

/// Start capturing described source columns; only a build arms it, so `sqb plan` records nothing.
pub(crate) fn arm_source_capture() {
    CAPTURED.with(|slot| *slot.borrow_mut() = Some(Vec::new()));
}

/// Keep one source's described SQL hash and column names when a build armed the capture.
pub(crate) fn capture_source_columns(source: &str, sql: &str, columns: &[String]) {
    CAPTURED.with(|slot| {
        if let Some(captured) = slot.borrow_mut().as_mut() {
            let mut value: Map<String, Value> = Map::new();
            value.insert(
                SOURCE_COLUMNS_SQL_HASH_KEY.to_owned(),
                Value::String(sha256_hex(sql.as_bytes())),
            );
            value.insert(
                SOURCE_COLUMNS_NAMES_KEY.to_owned(),
                Value::from(columns.to_vec()),
            );
            captured.push((source.to_owned(), Value::Object(value).to_string()));
        }
    });
}

/// The captured source columns, disarming the capture.
pub(crate) fn take_source_capture() -> DescribedSources {
    CAPTURED
        .with(|slot| slot.borrow_mut().take())
        .unwrap_or_default()
}
