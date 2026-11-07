//! The profile the harness measures ClickHouse under until `sqlquack-semantics` has its own.
//!
//! ClickHouse keeps every identifier as written and compares names exactly: they are case-sensitive,
//! quoted or not. Its name is not one `sqlquack-semantics` knows, so typing takes the generic rules
//! and the checks take the default engine policy: this measures SQLQuack as it stands.

use sqlquack_semantics::dialects::types::DialectProfile;

#[derive(Debug)]
pub struct ClickHouse;

pub const CLICKHOUSE: ClickHouse = ClickHouse;

impl DialectProfile for ClickHouse {
    fn name(&self) -> &'static str {
        "ClickHouse"
    }

    fn resolved_identifier(&self, name: &str, _quoted: bool) -> String {
        name.to_owned()
    }

    fn same_identifier(&self, left: &str, right: &str) -> bool {
        left == right
    }
}
