//! The declaration each orchestration authoring call makes, with the selections its conditions name.

use std::collections::BTreeMap;

use ruff_python_ast::{Expr, ExprCall};
use ruff_text_size::{Ranged, TextSize};

use crate::cpython_tree::main::walk_breadth_first::walk_breadth_first;
use crate::cpython_tree::models::{CpythonNode, ParsedSource, SourceLines};
use crate::orchestration_modules::_helpers::calls::{
    authoring_call, literal, literal_strings, module_names, position,
};
use crate::orchestration_modules::_helpers::conditions::{
    condition_source, module_assignments, module_imports,
};
use crate::orchestration_modules::constants::{
    JOB_KEYWORD, NAME_KEYWORD, RUN_EVENT_CALL, RUN_KEYWORD, TARGETS_KEYWORD, WHEN_KEYWORD,
};
use crate::orchestration_modules::models::{
    ConditionSource, LiteralArgument, ModuleScope, PythonDeclaration, PythonDeclarationKind,
};

/// The names `parsed`'s authoring calls and conditions resolve against, with `imports` from `imported_names`.
pub(crate) fn module_scope<'a>(
    parsed: &'a ParsedSource,
    contents: &'a str,
    imports: &'a [(String, String, TextSize)],
) -> ModuleScope<'a> {
    ModuleScope {
        imported: imports
            .iter()
            .map(|(local, name, _)| (local.as_str(), name.as_str()))
            .collect::<BTreeMap<&str, &str>>(),
        modules: module_names(&parsed.module.body),
        assignments: module_assignments(&parsed.module.body),
        imports: module_imports(&parsed.module.body),
        lines: &parsed.lines,
        contents,
    }
}

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
    let (job_references, condition) = if kind == PythonDeclarationKind::Sensor {
        sensor_references(call, scope)
    } else {
        (Vec::new(), None)
    };
    PythonDeclaration {
        kind,
        name,
        position: position(lines, call.range().start()),
        job_references,
        targets,
        condition,
    }
}

/// The sensor's `run` job, the jobs its run events name, and what its `when` condition selects.
fn sensor_references<'a>(
    call: &'a ExprCall,
    scope: &ModuleScope<'a>,
) -> (
    Vec<(&'static str, LiteralArgument)>,
    Option<ConditionSource>,
) {
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
    let condition: Option<ConditionSource> = call
        .arguments
        .find_keyword(WHEN_KEYWORD)
        .and_then(|when| condition_source(&when.value, call.range().start(), scope));
    (job_references, condition)
}
