//! Public traits and type aliases of analysis session.

/// `(succeeded, [(name, type, nullability)] or None, has star)`.
pub type ColumnsRow = (bool, Option<Vec<(String, Option<String>, String)>>, bool);
