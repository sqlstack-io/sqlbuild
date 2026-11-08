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

    /// A type spelt so the type fallback gives Polyglot's canonical name; `None` when empty.
    #[must_use]
    pub fn normalize_type(&self, spelling: &str) -> Option<String> {
        clickhouse_spelling(spelling)
    }

    /// A declared and an inferred type compared as ClickHouse's `toTypeName` spells them, wrappers
    /// apart: Polyglot's spellings wrap SQL aliases such as `INTEGER` in `Nullable`, the engine's don't.
    #[must_use]
    pub fn type_match(&self, declared: &str, inferred: &str, inferred_nullable: bool) -> TypeMatch {
        let spelled =
            |written: &str| clickhouse_type(written).unwrap_or_else(|| written.to_owned());
        clickhouse_type_match(&spelled(declared), &spelled(inferred), inferred_nullable)
    }
}
