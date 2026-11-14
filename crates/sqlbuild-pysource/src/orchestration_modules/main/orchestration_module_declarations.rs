//! The calendars and sensors a module declares, and the job names they use, without importing it.

use std::collections::BTreeMap;

use ruff_python_ast::Expr;
use ruff_text_size::{Ranged, TextSize};

use crate::cpython_tree::main::parse_cpython_module::parse_cpython_module;
use crate::cpython_tree::main::walk_breadth_first::walk_breadth_first;
use crate::cpython_tree::models::{CpythonNode, ParsedSource};
use crate::orchestration_modules::_helpers::calls::{
    authoring_call, imported_names, literal, position,
};
use crate::orchestration_modules::constants::{
    CALENDAR_CALL, JOB_KEYWORD, NAME_KEYWORD, REMOVED_NAMES, RUN_EVENT_CALL, RUN_KEYWORD,
    SENSOR_CALL,
};
use crate::orchestration_modules::models::{
    LiteralArgument, ModuleDeclarations, PythonDeclaration, PythonDeclarationKind,
};

/// Every `calendar(...)` and `sensor(...)` call with its name and job references, in source order; `None` when unparsable.
#[must_use]
pub fn orchestration_module_declarations(
    contents: &str,
    python_version: (u8, u8),
) -> Option<ModuleDeclarations> {
    let parsed: ParsedSource = parse_cpython_module(contents, python_version)?;
    let imports: Vec<(String, String, TextSize)> = imported_names(&parsed.module.body);
    let imported: BTreeMap<&str, &str> = imports
        .iter()
        .map(|(local, name, _)| (local.as_str(), name.as_str()))
        .collect();
    let removed_imports = imports
        .iter()
        .filter(|(_, name, _)| REMOVED_NAMES.contains(&name.as_str()))
        .map(|(_, name, start)| (name.clone(), position(&parsed.lines, *start)))
        .collect();
    let mut declarations: Vec<PythonDeclaration> = Vec::new();
    for node in walk_breadth_first(parsed.module.body.iter().map(CpythonNode::Stmt).collect()) {
        let CpythonNode::Expr(Expr::Call(call)) = node else {
            continue;
        };
        let kind = match authoring_call(call, &imported) {
            Some(CALENDAR_CALL) => PythonDeclarationKind::Calendar,
            Some(SENSOR_CALL) => PythonDeclarationKind::Sensor,
            _ => continue,
        };
        let name: Option<LiteralArgument> = call
            .arguments
            .find_argument_value(NAME_KEYWORD, 0)
            .map(|value| literal(value, &parsed.lines));
        let mut job_references: Vec<(&'static str, LiteralArgument)> = Vec::new();
        if kind == PythonDeclarationKind::Sensor {
            if let Some(run) = call.arguments.find_keyword(RUN_KEYWORD) {
                job_references.push((RUN_KEYWORD, literal(&run.value, &parsed.lines)));
            }
            let conditions: Vec<CpythonNode<'_>> = call
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
            for inner in walk_breadth_first(conditions) {
                let CpythonNode::Expr(Expr::Call(inner)) = inner else {
                    continue;
                };
                if authoring_call(inner, &imported) != Some(RUN_EVENT_CALL) {
                    continue;
                }
                if let Some(job) = inner.arguments.find_keyword(JOB_KEYWORD) {
                    job_references.push((RUN_EVENT_CALL, literal(&job.value, &parsed.lines)));
                }
            }
        }
        declarations.push(PythonDeclaration {
            kind,
            name,
            position: position(&parsed.lines, call.range().start()),
            job_references,
        });
    }
    declarations
        .sort_by_key(|declaration| (declaration.position.line, declaration.position.column));
    Some(ModuleDeclarations {
        declarations,
        removed_imports,
    })
}
