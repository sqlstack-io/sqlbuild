//! Public traits and type aliases of analysis.

/// `(name, schema, database)`.
pub type RelationRow = (String, Option<String>, Option<String>);

/// `(relations, temporary names)` named in a SQL text, or None if unparseable.
pub type ExtractedRows = Option<(Vec<RelationRow>, Vec<String>)>;
