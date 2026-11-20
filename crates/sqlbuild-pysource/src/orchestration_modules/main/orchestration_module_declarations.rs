//! The calendars and sensors a module declares, and the job names they use, without importing it.

use std::collections::BTreeMap;

use ruff_python_ast::{Expr, Stmt, StmtFunctionDef};
use ruff_text_size::{Ranged, TextSize};

use crate::cpython_tree::main::parse_cpython_module::parse_cpython_module;
use crate::cpython_tree::main::walk_breadth_first::walk_breadth_first;
use crate::cpython_tree::models::{CpythonNode, ParsedSource};
use crate::orchestration_modules::_helpers::calls::{
    authoring_call, authoring_name, imported_names, position,
};
use crate::orchestration_modules::_helpers::declarations::{
    bare_sensor, call_declaration, module_scope,
};
use crate::orchestration_modules::constants::{CALENDAR_CALL, REMOVED_NAMES, SENSOR_CALL};
use crate::orchestration_modules::models::{
    ModuleDeclarations, ModuleScope, PythonDeclaration, PythonDeclarationKind, SourcePosition,
};

/// Every `calendar(...)` call and `sensor` decorator with its name and job references, in source order; `None` when unparsable.
#[must_use]
pub fn orchestration_module_declarations(
    contents: &str,
    python_version: (u8, u8),
) -> Option<ModuleDeclarations> {
    let parsed: ParsedSource = parse_cpython_module(contents, python_version)?;
    let imports: Vec<(String, String, TextSize)> = imported_names(&parsed.module.body);
    let removed_imports: Vec<(String, SourcePosition)> = imports
        .iter()
        .filter(|(_, name, _)| REMOVED_NAMES.contains(&name.as_str()))
        .map(|(_, name, start)| (name.clone(), position(&parsed.lines, *start)))
        .collect();
    let scope: ModuleScope<'_> = module_scope(&parsed, contents, &imports);
    let mut declarations: Vec<PythonDeclaration> = Vec::new();
    let mut decorated: BTreeMap<TextSize, &StmtFunctionDef> = BTreeMap::new();
    let nodes: Vec<CpythonNode<'_>> =
        walk_breadth_first(parsed.module.body.iter().map(CpythonNode::Stmt).collect());
    for node in &nodes {
        let CpythonNode::Stmt(Stmt::FunctionDef(function)) = node else {
            continue;
        };
        for decorator in &function.decorator_list {
            match &decorator.expression {
                Expr::Call(call) => {
                    decorated.insert(call.range().start(), function);
                }
                bare if authoring_name(bare, (&scope.imported, &scope.modules))
                    == Some(SENSOR_CALL) =>
                {
                    declarations.push(bare_sensor(bare, function, scope.lines));
                }
                _ => {}
            }
        }
    }
    for node in nodes {
        let CpythonNode::Expr(Expr::Call(call)) = node else {
            continue;
        };
        let kind = match authoring_call(call, (&scope.imported, &scope.modules)) {
            Some(CALENDAR_CALL) => PythonDeclarationKind::Calendar,
            Some(SENSOR_CALL) => PythonDeclarationKind::Sensor,
            _ => continue,
        };
        let function = decorated.get(&call.range().start()).copied();
        declarations.push(call_declaration(call, kind, (&scope, function)));
    }
    declarations
        .sort_by_key(|declaration| (declaration.position.line, declaration.position.column));
    Some(ModuleDeclarations {
        declarations,
        removed_imports,
    })
}
