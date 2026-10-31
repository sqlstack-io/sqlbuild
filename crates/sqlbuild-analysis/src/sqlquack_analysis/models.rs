//! What SQLQuack analysis reads and returns for compiled models.

use crate::lineage::models::{LineageConfidence, LineageTransformKind};

/// One model as compile analysed it: its cleaned SQL, references and the schema it was bound to.
#[derive(Debug, Clone, Copy)]
pub struct CompiledModelSql<'a> {
    /// The cleaned analysis SQL, reading relations by their analysis names.
    pub sql: &'a str,
    /// `(analysis name, resource type, resource name)` of each relation it reads.
    pub references: &'a [(String, String, String)],
    /// Each relation's `(column, type)` pairs, a quoted column written with its quotes.
    pub schema: &'a [(String, Vec<(String, String)>)],
}

/// One output column's compile lineage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompileLineageColumn {
    pub output_column: String,
    pub transform: LineageTransformKind,
    pub confidence: LineageConfidence,
    /// `(resource type, resource name, column)`, the column as it resolves, sorted.
    pub sources: Vec<(String, String, String)>,
}
