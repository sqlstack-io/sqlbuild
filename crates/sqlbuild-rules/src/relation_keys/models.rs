/// One project relation's declared key facts: its incremental config and audited columns.
#[derive(Debug, Clone, Default)]
pub struct DeclaredRelation {
    /// `ref`, `seed` or `source`.
    pub kind: String,
    pub name: String,
    pub materialized: String,
    pub incremental_mode: String,
    pub incremental_strategy: String,
    pub has_cursor: bool,
    /// The configured `unique_key` columns, when it is a name or a list of names.
    pub unique_key: Option<Vec<String>>,
    pub columns: Vec<DeclaredKeyColumn>,
    /// Table-level audits as `(definition name, its string column argument)`.
    pub audits: Vec<(String, Option<String>)>,
    /// Whether the relation is known to hold at most one row, which the empty key proves.
    pub at_most_one_row: bool,
}

/// One declared column: its name, its audit definition names and whether it is declared non-null.
#[derive(Debug, Clone, Default)]
pub struct DeclaredKeyColumn {
    pub name: String,
    pub audits: Vec<String>,
    pub not_nullable: bool,
}
