//! Data used to match output columns across a migration.

use std::collections::BTreeSet;

/// One origin schema difference used by compatibility policy.
#[derive(Debug)]
pub struct CompatibilityFinding {
    pub kind: String,
    pub column_name: String,
    pub expected_type: Option<String>,
    pub actual_type: Option<String>,
}

/// Ordered candidates for automatic migration matching.
#[derive(Debug)]
pub struct MigrationCandidate {
    pub name: String,
    pub fingerprint: String,
}

/// Rename variants and unique candidate indexes after collision rejection.
#[derive(Debug)]
pub struct MigrationMatches {
    pub variants: Vec<std::collections::BTreeMap<String, String>>,
    pub matches: Vec<(String, usize)>,
    pub ambiguous: std::collections::BTreeMap<String, String>,
}

/// Matches and origin claims from one collision-elimination pass.
#[derive(Debug)]
pub struct MigrationMatchPass {
    pub result: MigrationMatches,
    pub contested: Vec<(String, Vec<String>)>,
}

/// A named projection with canonical expression and reference names.
#[derive(Debug)]
pub struct Projection {
    pub name: String,
    pub key: String,
    pub expression: String,
    pub references: BTreeSet<String>,
}

/// A suggestion to declare an explicit column migration.
#[derive(Debug)]
pub struct RenameHint {
    pub added_column: String,
    pub candidate_columns: Vec<String>,
    pub identical: bool,
}

/// One output expression and its alias binding facts.
#[derive(Debug)]
pub struct QueryProjection {
    pub name: String,
    pub expression: String,
    pub references: BTreeSet<String>,
    pub passthrough: bool,
}

/// One parsed statement as a fingerprint reads it: the syntax tree as JSON, whose identifiers
/// carry their source spans, and the source span of every token. Spans are Unicode character
/// offsets into the parsed SQL, end exclusive; tokens exclude whitespace and comments.
#[derive(Debug)]
pub struct ParsedStatement {
    pub tree: serde_json::Value,
    pub tokens: Vec<std::ops::Range<usize>>,
}

/// A plain SELECT with canonical expressions and input binding facts.
#[derive(Debug)]
pub struct QueryShape {
    pub projections: Vec<QueryProjection>,
    pub body: String,
    pub alias_clauses: String,
    pub positional: bool,
    pub input_relations: Option<Vec<(String, String)>>,
    pub local_input_columns: BTreeSet<String>,
    pub clause_references: BTreeSet<String>,
}
/// Display facts for a column migration warning.
#[derive(Debug)]
pub struct ColumnWarning<'a> {
    pub model_name: &'a str,
    pub relation: &'a str,
    pub origin: &'a str,
    pub destination: &'a str,
    pub completed: &'a str,
    pub target: &'a str,
    pub missing_outcome: &'a str,
    pub message: &'a str,
}
/// Display facts and ordered gates for one model migration diagnostic.
#[derive(Debug)]
pub struct ModelWarning<'a> {
    pub name: &'a str,
    pub origin: &'a str,
    pub destination: &'a str,
    pub decision: &'a str,
    pub manual: bool,
    pub completed: Option<&'a str>,
    pub target: &'a str,
    pub hidden: bool,
    pub missing: bool,
    pub deny: bool,
    pub blocked: bool,
    pub remedy: &'a str,
    pub outcome: &'a str,
    pub findings: &'a str,
}
