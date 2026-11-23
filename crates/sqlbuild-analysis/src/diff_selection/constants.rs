//! Diff selector syntax and the codes of its errors.

pub(crate) const TAG_PREFIX: &str = "tag:";
pub(crate) const PATH_PREFIX: &str = "path:";
pub(crate) const PATH_SEPARATORS: [char; 2] = ['/', '\\'];
pub(crate) const MODEL_PATH_ROOT: &str = "sql/models";
pub(crate) const MODEL_PATH_PREFIX: &str = "sql/models/";
pub(crate) const GRAPH_OPERATOR: char = '+';
pub(crate) const INTERSECTION_SEPARATOR: char = ',';
pub(crate) const EMPTY_SELECTOR_CODE: &str = "S001";
pub(crate) const UNKNOWN_NAME_CODE: &str = "S007";
pub(crate) const UNKNOWN_TAG_CODE: &str = "S008";
pub(crate) const UNKNOWN_PATH_CODE: &str = "S009";
pub(crate) const RAW_QUERY_READ_ONLY_ROOTS: [&str; 5] =
    ["except", "intersect", "select", "union", "values"];
