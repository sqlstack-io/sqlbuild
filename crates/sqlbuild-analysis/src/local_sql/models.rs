//! What converting one local scenario statement produced.

/// The DuckDB SQL or None, the result columns, and the refusal text, as the Python binding returns them.
pub type LocalSqlParts = (Option<String>, Vec<(String, String)>, String);

/// One statement as DuckDB SQL with its result columns, or SQLQuack's reasons for refusing it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LocalSql {
    /// The DuckDB SQL and its result columns as name and Snowflake type pairs.
    Converted {
        sql: String,
        columns: Vec<(String, String)>,
    },
    /// SQLQuack's diagnostics for the statement, rendered as its command line prints them.
    Refused(String),
}

impl LocalSql {
    /// The SQL, or None when refused; the columns, empty when refused; the refusal, empty when converted.
    pub fn into_parts(self) -> LocalSqlParts {
        match self {
            LocalSql::Converted { sql, columns } => (Some(sql), columns, String::new()),
            LocalSql::Refused(reason) => (None, Vec::new(), reason),
        }
    }
}
