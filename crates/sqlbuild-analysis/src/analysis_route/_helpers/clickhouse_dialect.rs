//! ClickHouse's native module: SQLQuack's ClickHouse parser, profile and type spellings.

use sqlquack_semantics::dialects::constants::CLICKHOUSE;
use sqlquack_semantics::dialects::main::clickhouse_type::clickhouse_type;
use sqlquack_semantics::dialects::types::DialectProfile;
use sqlquack_syntax::Parsed;
use sqlquack_syntax_clickhouse::parse_strict;

use crate::analysis_route::_helpers::clickhouse_spelling::clickhouse_spelling;
use crate::analysis_route::constants::{CLICKHOUSE_DIALECT, CLICKHOUSE_STUB_PREFIX};
use crate::analysis_route::main::clickhouse_type_match::clickhouse_type_match;
use crate::analysis_route::models::{ClickHouseDialect, ParserGap, TypeMatch};
use crate::analysis_route::types::NativeDialect;
use crate::type_system::models::NormalizedType;

impl NativeDialect for ClickHouseDialect {
    fn name(&self) -> &'static str {
        CLICKHOUSE_DIALECT
    }

    fn parse(&self, sql: &str) -> Result<Parsed, ParserGap> {
        parse_strict(sql).map_err(|error| ParserGap {
            construct: error.message,
            offset: error.position,
        })
    }

    fn stub_prefix(&self) -> &'static str {
        CLICKHOUSE_STUB_PREFIX
    }
}

impl ClickHouseDialect {
    /// The ClickHouse profile: names compared with case, types as ClickHouse 26.1 reports them.
    #[must_use]
    pub fn profile(&self) -> &'static dyn DialectProfile {
        &CLICKHOUSE
    }

    /// A type normalized as Polyglot normalizes it for ClickHouse; `None` when empty.
    #[must_use]
    pub fn normalize_type(&self, spelling: &str) -> Option<NormalizedType> {
        clickhouse_spelling(spelling)
    }

    /// Two types compared as `toTypeName` spells them, wrappers apart; `None` if either is refused.
    #[must_use]
    pub fn type_match(
        &self,
        declared: &str,
        inferred: &str,
        inferred_nullable: bool,
    ) -> Option<TypeMatch> {
        Some(clickhouse_type_match(
            &clickhouse_type(declared)?,
            &clickhouse_type(inferred)?,
            inferred_nullable,
        ))
    }
}
