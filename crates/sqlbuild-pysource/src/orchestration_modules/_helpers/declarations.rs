//! The declaration each orchestration authoring call makes, with the selections its conditions name.

use std::collections::BTreeMap;

use ruff_python_ast::{Expr, ExprCall, StmtFunctionDef};
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
    JOB_KEYWORD, NAME_KEYWORD, RUN_EVENT_CALL, RUN_REQUEST_CALL, TARGETS_KEYWORD, WHEN_KEYWORD,
};
use crate::orchestration_modules::models::{
    ConditionSource, LiteralArgument, ModuleScope, PythonDeclaration, PythonDeclarationKind,
    SensorForm,
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

/// The declaration one `calendar(...)` or `sensor(...)` call makes; `decorated` is the function a sensor call decorates.
pub(crate) fn call_declaration<'a>(
    call: &'a ExprCall,
    kind: PythonDeclarationKind,
    (scope, decorated): (&ModuleScope<'a>, Option<&'a StmtFunctionDef>),
) -> PythonDeclaration {
    let lines: &SourceLines = scope.lines;
    let sensor: bool = kind == PythonDeclarationKind::Sensor;
    let name: Option<LiteralArgument> = if sensor {
        call.arguments
            .find_keyword(NAME_KEYWORD)
            .map(|keyword| literal(&keyword.value, lines))
            .or_else(|| {
                decorated.map(|function| LiteralArgument {
                    value: Some(function.name.as_str().to_owned()),
                    position: position(lines, function.name.range().start()),
                })
            })
    } else {
        call.arguments
            .find_argument_value(NAME_KEYWORD, 0)
            .map(|value| literal(value, lines))
    };
    let targets: Option<Vec<String>> = match call.arguments.find_keyword(TARGETS_KEYWORD) {
        Some(targets) => literal_strings(&targets.value),
        None => Some(Vec::new()),
    };
    let (job_references, condition) = if sensor {
        sensor_references(call, (scope, decorated))
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
        sensor_form: sensor.then(|| match decorated {
            Some(function) => SensorForm::Decorator {
                function: function.name.as_str().to_owned(),
            },
            None => SensorForm::Call,
        }),
        positional: call
            .arguments
            .args
            .first()
            .filter(|_| sensor)
            .map(|argument| position(lines, argument.range().start())),
    }
}

/// A bare `@sensor` decorator, which declares nothing until it is called with its arguments.
pub(crate) fn bare_sensor(
    at: &Expr,
    function: &StmtFunctionDef,
    lines: &SourceLines,
) -> PythonDeclaration {
    PythonDeclaration {
        kind: PythonDeclarationKind::Sensor,
        name: Some(LiteralArgument {
            value: Some(function.name.as_str().to_owned()),
            position: position(lines, function.name.range().start()),
        }),
        position: position(lines, at.range().start()),
        job_references: Vec::new(),
        targets: Some(Vec::new()),
        condition: None,
        sensor_form: Some(SensorForm::BareDecorator),
        positional: None,
    }
}

/// The jobs a sensor's run events name and its function's literal `RunRequest(job=...)` starts, and what `when` selects.
fn sensor_references<'a>(
    call: &'a ExprCall,
    (scope, decorated): (&ModuleScope<'a>, Option<&'a StmtFunctionDef>),
) -> (
    Vec<(&'static str, LiteralArgument)>,
    Option<ConditionSource>,
) {
    let lines: &SourceLines = scope.lines;
    let mut job_references: Vec<(&'static str, LiteralArgument)> = Vec::new();
    let conditions: Vec<CpythonNode<'a>> = call
        .arguments
        .keywords
        .iter()
        .map(|keyword| CpythonNode::Expr(&keyword.value))
        .collect();
    for inner in walk_breadth_first(conditions) {
        let CpythonNode::Expr(Expr::Call(inner)) = inner else {
            continue;
        };
        if authoring_call(inner, (&scope.imported, &scope.modules)) == Some(RUN_EVENT_CALL)
            && let Some(job) = inner.arguments.find_keyword(JOB_KEYWORD)
        {
            job_references.push((RUN_EVENT_CALL, literal(&job.value, lines)));
        }
    }
    let body: Vec<CpythonNode<'a>> = decorated
        .into_iter()
        .flat_map(|function| function.body.iter().map(CpythonNode::Stmt))
        .collect();
    for inner in walk_breadth_first(body) {
        let CpythonNode::Expr(Expr::Call(inner)) = inner else {
            continue;
        };
        if authoring_call(inner, (&scope.imported, &scope.modules)) == Some(RUN_REQUEST_CALL)
            && let Some(job) = inner.arguments.find_keyword(JOB_KEYWORD)
            && matches!(job.value, Expr::StringLiteral(_))
        {
            job_references.push((RUN_REQUEST_CALL, literal(&job.value, lines)));
        }
    }
    let condition: Option<ConditionSource> = call
        .arguments
        .find_keyword(WHEN_KEYWORD)
        .and_then(|when| condition_source(&when.value, call.range().start(), scope));
    (job_references, condition)
}
