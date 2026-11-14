//! The declaration each orchestration authoring call makes, with the selections its conditions name.

use ruff_python_ast::{Expr, ExprCall};
use ruff_text_size::Ranged;

use crate::cpython_tree::main::walk_breadth_first::walk_breadth_first;
use crate::cpython_tree::models::{CpythonNode, SourceLines};
use crate::orchestration_modules::_helpers::calls::{
    authoring_call, literal, literal_strings, position,
};
use crate::orchestration_modules::_helpers::conditions::condition_nodes;
use crate::orchestration_modules::constants::{
    JOB_KEYWORD, NAME_KEYWORD, RUN_EVENT_CALL, RUN_KEYWORD, SELECT_KEYWORD,
    SELECTING_CONDITION_CALLS, TARGETS_KEYWORD,
};
use crate::orchestration_modules::models::{
    LiteralArgument, ModuleScope, PythonDeclaration, PythonDeclarationKind,
};

/// The declaration one `calendar(...)` or `sensor(...)` call makes.
pub(crate) fn call_declaration<'a>(
    call: &'a ExprCall,
    kind: PythonDeclarationKind,
    scope: &ModuleScope<'a>,
) -> PythonDeclaration {
    let lines: &SourceLines = scope.lines;
    let name: Option<LiteralArgument> = call
        .arguments
        .find_argument_value(NAME_KEYWORD, 0)
        .map(|value| literal(value, lines));
    let targets: Option<Vec<String>> = match call.arguments.find_keyword(TARGETS_KEYWORD) {
        Some(targets) => literal_strings(&targets.value),
        None => Some(Vec::new()),
    };
    let (job_references, selections) = if kind == PythonDeclarationKind::Sensor {
        sensor_references(call, scope)
    } else {
        (Vec::new(), Vec::new())
    };
    PythonDeclaration {
        kind,
        name,
        position: position(lines, call.range().start()),
        job_references,
        targets,
        selections,
    }
}

/// The sensor's `run` job, the jobs its run events name, and the `select` of each condition that selects nodes.
fn sensor_references<'a>(
    call: &'a ExprCall,
    scope: &ModuleScope<'a>,
) -> (Vec<(&'static str, LiteralArgument)>, Vec<LiteralArgument>) {
    let lines: &SourceLines = scope.lines;
    let mut job_references: Vec<(&'static str, LiteralArgument)> = Vec::new();
    if let Some(run) = call.arguments.find_keyword(RUN_KEYWORD) {
        job_references.push((RUN_KEYWORD, literal(&run.value, lines)));
    }
    let conditions: Vec<CpythonNode<'a>> = call
        .arguments
        .keywords
        .iter()
        .filter(|keyword| {
            keyword
                .arg
                .as_ref()
                .is_some_and(|arg| arg.as_str() != RUN_KEYWORD)
        })
        .map(|keyword| CpythonNode::Expr(&keyword.value))
        .collect();
    for inner in walk_breadth_first(conditions.clone()) {
        let CpythonNode::Expr(Expr::Call(inner)) = inner else {
            continue;
        };
        if authoring_call(inner, (&scope.imported, &scope.modules)) == Some(RUN_EVENT_CALL)
            && let Some(job) = inner.arguments.find_keyword(JOB_KEYWORD)
        {
            job_references.push((RUN_EVENT_CALL, literal(&job.value, lines)));
        }
    }
    let selections: Vec<LiteralArgument> = condition_nodes(conditions, &scope.assignments)
        .into_iter()
        .filter_map(|node| condition_selection(node, scope))
        .collect();
    (job_references, selections)
}

/// The `select` of a node-selecting condition call; one with no `select` keyword, such as `**options`, cannot be counted.
fn condition_selection(node: CpythonNode<'_>, scope: &ModuleScope<'_>) -> Option<LiteralArgument> {
    let lines: &SourceLines = scope.lines;
    let CpythonNode::Expr(Expr::Call(call)) = node else {
        return None;
    };
    let name: &str = authoring_call(call, (&scope.imported, &scope.modules))?;
    if !SELECTING_CONDITION_CALLS.contains(&name) {
        return None;
    }
    Some(match call.arguments.find_keyword(SELECT_KEYWORD) {
        Some(select) => literal(&select.value, lines),
        None => LiteralArgument {
            value: None,
            position: position(lines, call.range().start()),
        },
    })
}
