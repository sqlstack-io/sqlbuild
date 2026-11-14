//! Row readers shared by the control-store schema contracts.

use crate::store::types::Row;

pub(crate) fn text(row: &Row, key: &str) -> String {
    row[key].as_str().expect("text column").to_owned()
}
