//! Keep one source's described SQL hash and column names when a build armed the capture.

use crate::observations::_helpers::source_capture::CAPTURED;
use serde_json::{Map, Value};
use sqlbuild_cache::digest::main::sha256_hex::sha256_hex;
use sqlbuild_state::observations::constants::{
    SOURCE_COLUMNS_NAMES_KEY, SOURCE_COLUMNS_SQL_HASH_KEY,
};

/// Keep one source's described SQL hash and column names when a build armed the capture.
pub fn capture_source_columns(source: &str, sql: &str, columns: &[String]) {
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
