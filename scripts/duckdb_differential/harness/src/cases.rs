//! The SQL bodies `scripts/run_duckdb_differential.py` collects.

use serde::Deserialize;

/// One SQL body SQLBuild parses or emits for DuckDB.
#[derive(Debug, Deserialize)]
pub struct Case {
    /// `<source>/<project>/<kind>/<name>`.
    pub id: String,
    /// The corpus label the body came from.
    pub source: String,
    /// What the body is: `model`, `hook`, `audit`, `sql_test`, `function`, `template`, ...
    pub kind: String,
    /// The SQL text, with SQLBuild's reference markers where the compile keeps them.
    pub sql: String,
    /// Whether SQLBuild's gate parses the body as exactly one statement (`parse`) or validates
    /// it (`validate`); emitted SQL that SQLBuild never parses is `none`.
    pub mode: String,
    /// The text SQLQuack parses instead, when DuckDB only ever receives the body wrapped.
    #[serde(default)]
    pub sqlquack_sql: Option<String>,
}
