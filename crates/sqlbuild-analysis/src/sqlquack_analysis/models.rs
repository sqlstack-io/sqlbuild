//! What SQLQuack analysis reads and returns for compiled models.

use sqlquack_semantics::lineage::models::SchemaColumn;

use crate::lineage::models::{LineageConfidence, LineageTransformKind};
use crate::sqlquack_analysis::types::TracedModel;

/// One model as compile analysed it: its cleaned SQL, references and the schema it was bound to.
#[derive(Debug, Clone, Copy)]
pub struct CompiledModelSql<'a> {
    /// The cleaned analysis SQL, reading relations by their analysis names.
    pub sql: &'a str,
    /// `(analysis name, resource type, resource name)` of each relation it reads.
    pub references: &'a [(String, String, String)],
    /// Each relation's `(column, type)` pairs, a quoted column written with its quotes.
    pub schema: &'a [(String, Vec<(String, String)>)],
    /// Each relation's `(column, type)` pairs the catalog knows, its column list complete or not.
    pub types: &'a [(String, Vec<(String, String)>)],
    /// Each relation's `(column, nullability)` pairs, as the catalog knows them.
    pub nullability: &'a [(String, Vec<(String, String)>)],
}

/// One compiled model's lineage: its output columns and where its stars stand.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompiledLineage {
    pub columns: Vec<CompileLineageColumn>,
    /// Whether the outermost query level projects a star.
    pub projects_star: bool,
    /// Whether a star over a relation of unknown columns left outputs out.
    pub open_star: bool,
}

/// One output column's compile lineage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompileLineageColumn {
    pub output_column: String,
    pub transform: LineageTransformKind,
    pub confidence: LineageConfidence,
    /// `(resource type, resource name, column)`, the column as it resolves, sorted.
    pub sources: Vec<(String, String, String)>,
    /// The output's type, when SQLQuack knows it.
    pub data_type: Option<String>,
    /// `non_null`, `nullable` or `unknown`.
    pub nullability: &'static str,
}

/// One learning pass of rich lineage: what it traced, the outputs it learnt, the models to retrace.
#[derive(Debug)]
pub(crate) struct LearningPass {
    pub(crate) traced: Vec<TracedModel>,
    pub(crate) learnt: Vec<(String, Vec<SchemaColumn>)>,
    pub(crate) readers: Vec<usize>,
}

/// What the adapter knows of functions, read for output types and nullability.
#[derive(Debug, Clone, Copy, Default)]
pub struct CompileFunctions<'a> {
    /// `(function, return type)`.
    pub return_types: &'a [(String, String)],
    /// `(function, rule id)`.
    pub nullability_rules: &'a [(String, String)],
}
