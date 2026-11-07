//! The profile the harness measures SQL Server under until `sqlquack-semantics` has its own.
//!
//! SQLBuild's SQL Server adapter sets no collation, so names follow the server default,
//! SQL_Latin1_General_CP1_CI_AS: case-insensitive, delimited or not. Its name is not one
//! `sqlquack-semantics` knows, so typing takes the generic rules and the checks take the default
//! engine policy: this measures SQLQuack as it stands.

use sqlquack_semantics::dialects::types::DialectProfile;

#[derive(Debug)]
pub struct SqlServer;

pub const SQL_SERVER: SqlServer = SqlServer;

impl DialectProfile for SqlServer {
    fn name(&self) -> &'static str {
        "SQL Server"
    }

    fn resolved_identifier(&self, name: &str, _quoted: bool) -> String {
        name.to_lowercase()
    }

    fn same_identifier(&self, left: &str, right: &str) -> bool {
        left.to_lowercase() == right.to_lowercase()
    }
}
