//! The code of a module that importing it never runs: lambda bodies and the `__main__` block.

use ruff_python_ast::Stmt;
use ruff_python_ast::visitor::Visitor;
use ruff_text_size::{Ranged, TextRange};

use crate::cpython_tree::main::parse_cpython_module::parse_cpython_module;
use crate::cpython_tree::models::ParsedSource;
use crate::node_units::_helpers::deferred::{Spans, runs_as_main};
use crate::node_units::models::DeferredSpan;

/// Each lambda body and the body of a top-level `if __name__ == "__main__":`, or `None` when `contents` does not parse.
#[must_use]
pub fn deferred_spans(contents: &str, python_version: (u8, u8)) -> Option<Vec<DeferredSpan>> {
    let parsed: ParsedSource = parse_cpython_module(contents, python_version)?;
    let mut spans = Spans {
        lines: &parsed.lines,
        spans: Vec::new(),
    };
    for statement in &parsed.module.body {
        if let Stmt::If(block) = statement
            && runs_as_main(&block.test)
            && let (Some(first), Some(last)) = (block.body.first(), block.body.last())
        {
            spans.push(TextRange::new(first.start(), last.end()));
            for clause in &block.elif_else_clauses {
                spans.visit_elif_else_clause(clause);
            }
            continue;
        }
        spans.visit_stmt(statement);
    }
    Some(spans.spans)
}
