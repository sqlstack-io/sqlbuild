//! `deeper_than`: whether a parsed statement nests past a depth limit.

use sqlquack_syntax::{Expression, ExpressionWalk};

/// Whether `statement` nests more than `limit` levels, walked iteratively so depth cannot overflow.
pub fn deeper_than(statement: &Expression, limit: usize) -> bool {
    let mut stack: Vec<(&Expression, usize)> = vec![(statement, 1)];
    while let Some((node, depth)) = stack.pop() {
        if depth > limit {
            return true;
        }
        stack.extend(node.children().into_iter().map(|child| (child, depth + 1)));
    }
    false
}
