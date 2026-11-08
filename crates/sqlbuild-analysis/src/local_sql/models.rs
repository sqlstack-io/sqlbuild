//! What converting one local scenario statement produced.

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
