//! ClickHouse's native module: SQLQuack's ClickHouse parser, profile and type spellings.

use sqlquack_semantics::dialects::constants::CLICKHOUSE;
use sqlquack_semantics::dialects::main::clickhouse_type::clickhouse_type;
use sqlquack_semantics::dialects::types::DialectProfile;
use sqlquack_syntax::Parsed;
use sqlquack_syntax_clickhouse::parse_strict;

use crate::analysis_route::_helpers::clickhouse_spelling::clickhouse_spelling;
use crate::analysis_route::constants::{
    CLICKHOUSE_DIALECT, CLICKHOUSE_STUB_PREFIX, STATEMENTS_CONSTRUCT,
};
use crate::analysis_route::main::clickhouse_type_match::clickhouse_type_match;
use crate::analysis_route::models::{ClickHouseDialect, ParserGap, TypeMatch};
use crate::analysis_route::types::NativeDialect;
use crate::type_system::models::NormalizedType;

impl NativeDialect for ClickHouseDialect {
    fn name(&self) -> &'static str {
        CLICKHOUSE_DIALECT
    }

    fn read_whole(&self, sql: &str) -> Result<(), ParserGap> {
        let parsed: Parsed = self.parse(sql)?;
        match (parsed.gaps.first(), parsed.statements.len()) {
            (Some(gap), _) => Err(ParserGap {
                construct: gap.construct.to_owned(),
                offset: gap.start,
            }),
            (None, 1) => Ok(()),
            (None, _) => Err(ParserGap {
                construct: STATEMENTS_CONSTRUCT.to_owned(),
                offset: sql.len(),
            }),
        }
    }

    fn stub_prefix(&self) -> &'static str {
        CLICKHOUSE_STUB_PREFIX
    }
}

impl ClickHouseDialect {
    /// Every statement of `sql` with the parser's named gaps, or the first error.
    pub fn parse(&self, sql: &str) -> Result<Parsed, ParserGap> {
        parse_strict(sql).map_err(|error| ParserGap {
            construct: error.message,
            offset: error.position,
        })
    }

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

    /// A declared and an inferred type compared as ClickHouse's `toTypeName` spells them, wrappers
    /// apart: Polyglot's spellings wrap SQL aliases such as `INTEGER` in `Nullable`, the engine's
    /// don't. `None`, not compared, when either side is no type ClickHouse accepts.
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
