//! The cases `scripts/bq_differential/semantic_cases.py` extracts from compile captures.

use serde::Deserialize;

/// One BigQuery query: its analysis SQL, the relations it reads and, for a compiled model,
/// SQLBuild's recorded results.
#[derive(Debug, Deserialize)]
pub struct Case {
    /// `<project>/<model>`, with `#<mutation>` for a mutated query.
    pub id: String,
    /// The query SQL with SQLBuild's reference markers, as the compile analysed it.
    pub query_sql: String,
    /// The model's `@@@` placeholder values, which analysis normalization substitutes.
    #[serde(default)]
    pub placeholders: std::collections::HashMap<String, String>,
    /// The relations the compile's binding catalog knew, by analysis name.
    pub relations: Vec<Relation>,
    /// The project's functions, as SQLBuild declares them to Polyglot.
    pub functions: Vec<Function>,
    /// What the compile recorded for the model; absent for a mutated query, which only
    /// measures name findings.
    pub polyglot: Option<PolyglotResult>,
}

/// One relation of the binding catalog and the resource it stands for, when a reference names it.
#[derive(Debug, Deserialize)]
pub struct Relation {
    pub name: String,
    pub resource_type: Option<String>,
    pub resource_name: Option<String>,
    /// A referenced relation the capture has no closed shape for: SQLBuild treats it as an
    /// open input, so it is left out of both schemas and only maps lineage to its resource.
    #[serde(default)]
    pub open: bool,
    pub columns: Vec<Column>,
}

/// A project function: scalar functions carry their return type, table functions none.
#[derive(Debug, Deserialize)]
pub struct Function {
    pub name: String,
    pub returns: Option<String>,
}

/// A column with its type spelling and inferred nullability (`non_null`, `nullable`, `unknown`).
#[derive(Debug, Deserialize)]
pub struct Column {
    pub name: String,
    pub data_type: Option<String>,
    pub nullability: String,
}

/// The compiled model's analysis facts.
#[derive(Debug, Deserialize)]
pub struct PolyglotResult {
    /// The inferred output columns; `None` when the analysis did not infer them.
    pub columns: Option<Vec<Column>>,
    pub lineage: Vec<LineageColumn>,
    pub has_star: bool,
}

/// One output column's lineage.
#[derive(Debug, Deserialize)]
pub struct LineageColumn {
    pub output: String,
    pub kind: String,
    pub sources: Vec<Source>,
}

/// One upstream resource column.
#[derive(Debug, Deserialize, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Source {
    pub resource_type: String,
    pub resource_name: String,
    pub column: String,
}
