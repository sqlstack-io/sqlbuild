//! BigQuery's native module: SQLQuack's GoogleSQL parser decides whether a model is read whole.

use sqlquack_syntax_bigquery::{Parsed, StrictFailure, parse_strict};

use crate::analysis_route::constants::{
    BIGQUERY_DIALECT, BIGQUERY_STUB_PREFIX, STATEMENTS_CONSTRUCT, UNSCANNED_CONSTRUCT,
};
use crate::analysis_route::models::{BigQueryDialect, ParserGap};
use crate::analysis_route::types::NativeDialect;

impl NativeDialect for BigQueryDialect {
    fn name(&self) -> &'static str {
        BIGQUERY_DIALECT
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
        BIGQUERY_STUB_PREFIX
    }
}
