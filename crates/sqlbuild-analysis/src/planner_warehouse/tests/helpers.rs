//! Shared builders for warehouse-inspection planning tests.

use crate::planner_warehouse::models::{CursorTagRequest, MissingSourceTable};

/// A cursor tag request on the `updated_at` column.
pub(super) fn request(relation: &str, min_tag: Option<&str>, max_tag: &str) -> CursorTagRequest {
    CursorTagRequest {
        relation: relation.to_owned(),
        cursor_column: "updated_at".to_owned(),
        cursor_type: None,
        min_tag: min_tag.map(str::to_owned),
        max_tag: max_tag.to_owned(),
    }
}

/// A missing source table read by `read_by`, unreadable when a role is given.
pub(super) fn missing(name: &str, read_by: &[&str], role: Option<&str>) -> MissingSourceTable {
    MissingSourceTable {
        source_name: name.to_owned(),
        relation: format!("raw.{name}"),
        read_by: read_by.iter().map(|reader| (*reader).to_owned()).collect(),
        unreadable: role.is_some(),
        role: role.map(str::to_owned),
        location: format!("sql/sources/{name}.yml:2"),
    }
}
