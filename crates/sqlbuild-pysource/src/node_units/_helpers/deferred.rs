//! The visitor that finds lambda bodies, and the `__main__` test.

use ruff_python_ast::visitor::{Visitor, walk_expr, walk_stmt};
use ruff_python_ast::{CmpOp, Expr, Stmt};
use ruff_text_size::{Ranged, TextRange, TextSize};

use crate::cpython_tree::main::source_position::source_position;
use crate::cpython_tree::models::SourceLines;
use crate::node_units::models::DeferredSpan;

/// Whether `test` is `__name__ == "__main__"`, either way round.
pub(crate) fn runs_as_main(test: &Expr) -> bool {
    let Expr::Compare(compare) = test else {
        return false;
    };
    let ([CmpOp::Eq], [right]) = (&*compare.ops, &*compare.comparators) else {
        return false;
    };
    let name = |expr: &Expr| matches!(expr, Expr::Name(name) if name.id.as_str() == "__name__");
    let main = |expr: &Expr| matches!(expr, Expr::StringLiteral(text) if text.value.to_str() == "__main__");
    (name(&compare.left) && main(right)) || (main(&compare.left) && name(right))
}

/// Collects lambda bodies without descending into them.
pub(crate) struct Spans<'a> {
    pub(crate) lines: &'a SourceLines,
    pub(crate) spans: Vec<DeferredSpan>,
}

impl Spans<'_> {
    /// Record `range` as deferred.
    pub(crate) fn push(&mut self, range: TextRange) {
        let position = |offset: TextSize| source_position(self.lines, offset);
        let span = DeferredSpan {
            start: position(range.start()),
            end: position(range.end()),
        };
        self.spans.push(span);
    }
}

impl<'a> Visitor<'a> for Spans<'_> {
    fn visit_stmt(&mut self, statement: &'a Stmt) {
        walk_stmt(self, statement);
    }

    fn visit_expr(&mut self, expr: &'a Expr) {
        if let Expr::Lambda(lambda) = expr {
            if let Some(parameters) = &lambda.parameters {
                self.visit_parameters(parameters);
            }
            self.push(lambda.body.range());
            return;
        }
        walk_expr(self, expr);
    }
}
