//! The whole-read rule every native arm shares: one statement and no gap.

use sqlquack_syntax::{Expression, Gap};

use crate::analysis_route::constants::{COMMAND_CONSTRUCT, STATEMENTS_CONSTRUCT};
use crate::analysis_route::models::ParserGap;

/// `Ok` when `statements` is one parsed statement and `gaps` is empty, else the first obstacle.
pub(crate) fn one_statement(
    statements: &[Expression],
    gaps: &[Gap],
    length: usize,
) -> Result<(), ParserGap> {
    if let Some(gap) = gaps.first() {
        return Err(ParserGap {
            construct: gap.construct.to_owned(),
            message: gap.construct.to_owned(),
            offset: gap.start,
            syntax: false,
        });
    }
    match statements {
        [Expression::Command(_)] => Err(ParserGap {
            construct: COMMAND_CONSTRUCT.to_owned(),
            message: COMMAND_CONSTRUCT.to_owned(),
            offset: 0,
            syntax: false,
        }),
        [_] => Ok(()),
        _ => Err(ParserGap {
            construct: STATEMENTS_CONSTRUCT.to_owned(),
            message: STATEMENTS_CONSTRUCT.to_owned(),
            offset: length,
            syntax: false,
        }),
    }
}
