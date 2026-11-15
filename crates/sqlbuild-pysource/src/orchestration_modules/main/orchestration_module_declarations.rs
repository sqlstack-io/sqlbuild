//! The calendars and sensors a module declares, and the job names they use, without importing it.

use ruff_python_ast::Expr;
use ruff_text_size::TextSize;

use crate::cpython_tree::main::parse_cpython_module::parse_cpython_module;
use crate::cpython_tree::main::walk_breadth_first::walk_breadth_first;
use crate::cpython_tree::models::{CpythonNode, ParsedSource};
use crate::orchestration_modules::_helpers::calls::{authoring_call, imported_names, position};
use crate::orchestration_modules::_helpers::declarations::{call_declaration, module_scope};
use crate::orchestration_modules::constants::{CALENDAR_CALL, REMOVED_NAMES, SENSOR_CALL};
use crate::orchestration_modules::models::{
    ModuleDeclarations, ModuleScope, PythonDeclaration, PythonDeclarationKind, SourcePosition,
};

/// Every `calendar(...)` and `sensor(...)` call with its name and job references, in source order; `None` when unparsable.
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
    for node in walk_breadth_first(parsed.module.body.iter().map(CpythonNode::Stmt).collect()) {
        let CpythonNode::Expr(Expr::Call(call)) = node else {
            continue;
        };
        let kind = match authoring_call(call, (&scope.imported, &scope.modules)) {
            Some(CALENDAR_CALL) => PythonDeclarationKind::Calendar,
            Some(SENSOR_CALL) => PythonDeclarationKind::Sensor,
            _ => continue,
        };
        declarations.push(call_declaration(call, kind, &scope));
    }
    declarations
        .sort_by_key(|declaration| (declaration.position.line, declaration.position.column));
    Some(ModuleDeclarations {
        declarations,
        removed_imports,
    })
}
