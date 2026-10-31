//! Internal result rows use native JSON scalars, with UUIDs and timestamps as strings.

pub(crate) type Row = std::collections::BTreeMap<String, serde_json::Value>;
