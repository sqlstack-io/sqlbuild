//! SQLite's native module: SQLQuack's SQLite parser decides whether a model is read whole.

use sqlquack_syntax_sqlite::{Parsed, StrictFailure, parse_strict};

use crate::analysis_route::_helpers::whole::one_statement;
use crate::analysis_route::constants::{SQLITE_DIALECT, UNQUOTED_STUB_PREFIX, UNSCANNED_CONSTRUCT};
use crate::analysis_route::models::{ParserGap, SqliteDialect};
use crate::analysis_route::types::NativeDialect;

impl NativeDialect for SqliteDialect {
    fn name(&self) -> &'static str {
        SQLITE_DIALECT
    }

    fn read_whole(&self, sql: &str) -> Result<(), ParserGap> {
        let parsed: Parsed = parse_strict(sql).map_err(|failure| match failure {
            StrictFailure::Parse(error) => ParserGap {
                construct: error.message,
                offset: error.position,
            },
            StrictFailure::Uncovered { position } => ParserGap {
                construct: UNSCANNED_CONSTRUCT.to_owned(),
                offset: position,
            },
        })?;
        one_statement(&parsed.statements, &parsed.gaps, sql.len())
    }

    fn stub_prefix(&self) -> &'static str {
        UNQUOTED_STUB_PREFIX
    }
}
