//! The profile the harness measures BigQuery under until `sqlquack-semantics` has its own.
//!
//! BigQuery keeps a column or alias as written and compares it without regard to case. Its name
//! is not one `sqlquack-semantics` knows, so typing takes the generic rules and the checks take
//! the default engine policy: this measures SQLQuack as it stands, without BigQuery rules.

use sqlquack_semantics::dialects::types::DialectProfile;

#[derive(Debug)]
pub struct BigQuery;

pub const BIGQUERY: BigQuery = BigQuery;

impl DialectProfile for BigQuery {
    fn name(&self) -> &'static str {
        "BigQuery"
    }

    fn resolved_identifier(&self, name: &str, _quoted: bool) -> String {
        name.to_owned()
    }

    fn same_identifier(&self, left: &str, right: &str) -> bool {
        left.eq_ignore_ascii_case(right)
    }
}
