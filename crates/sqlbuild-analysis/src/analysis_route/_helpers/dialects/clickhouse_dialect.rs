//! ClickHouse's native module: SQLQuack's ClickHouse parser, profile and type spellings.

use sqlquack_semantics::dialects::constants::CLICKHOUSE;
use sqlquack_semantics::dialects::types::DialectProfile;
use sqlquack_syntax::Parsed;
use sqlquack_syntax_clickhouse::parse_strict;

use crate::analysis_route::_helpers::dialects::clickhouse_types::{
    clickhouse_spelling, clickhouse_type, clickhouse_type_match,
};
use crate::analysis_route::constants::{CLICKHOUSE_DIALECT, UNQUOTED_STUB_PREFIX};
use crate::analysis_route::models::{ClickHouseDialect, ParserGap, TypeMatch};
use crate::analysis_route::types::NativeDialect;
use crate::type_system::models::NormalizedType;

impl NativeDialect for ClickHouseDialect {
    fn name(&self) -> &'static str {
        CLICKHOUSE_DIALECT
    }

    fn stub_prefix(&self) -> &'static str {
        UNQUOTED_STUB_PREFIX
    }

    fn parse(&self, sql: &str) -> Result<Parsed, ParserGap> {
        parse_strict(sql).map_err(|error| ParserGap {
            construct: error.message.clone(),
            message: error.message,
            offset: error.position,
            syntax: error.syntax,
        })
    }

    fn profile(&self) -> &'static dyn DialectProfile {
        &CLICKHOUSE
    }

    /// A type normalized as Polyglot normalizes it for ClickHouse; `None` when empty.
    fn normalize_type(&self, spelling: &str) -> Option<NormalizedType> {
        clickhouse_spelling(spelling)
    }

    /// Both sides respelt as ClickHouse's `toTypeName` prints them, then compared wrappers apart.
    fn type_match(
        &self,
        declared: &str,
        inferred: Option<&str>,
        inferred_nullable: bool,
    ) -> TypeMatch {
        let Some(declared) = clickhouse_type(declared) else {
            return TypeMatch::NotCompared {
                nullable_into_non_null: false,
            };
        };
        let inferred: Option<String> = inferred.and_then(clickhouse_type);
        clickhouse_type_match(&declared, inferred.as_deref(), inferred_nullable)
    }
}
