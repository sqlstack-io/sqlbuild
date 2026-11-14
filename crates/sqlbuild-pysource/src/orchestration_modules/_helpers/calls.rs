//! Orchestration authoring calls and their literal arguments, read from the ruff tree.

use std::collections::BTreeMap;

use ruff_python_ast::{Expr, ExprCall, Stmt};
use ruff_text_size::{Ranged, TextSize};

use crate::cpython_tree::main::source_position::source_position;
use crate::cpython_tree::models::SourceLines;
use crate::orchestration_modules::constants::{
    CALENDAR_CALL, ORCHESTRATION_MODULE, RUN_EVENT_CALL, SENSOR_CALL,
};
use crate::orchestration_modules::models::{LiteralArgument, SourcePosition};

/// The calls a module's `calendar`, `sensor` and `run_event` names can make.
const AUTHORING_CALLS: [&str; 3] = [CALENDAR_CALL, SENSOR_CALL, RUN_EVENT_CALL];

/// The 1-based line and column of `offset`.
pub(crate) fn position(lines: &SourceLines, offset: TextSize) -> SourcePosition {
    let (line, column) = source_position(lines, offset);
    SourcePosition {
        line,
        column: column + 1,
    }
}

/// Each local name a top-level `from sqlbuild.orchestration import ...` binds, with the name it imports and where.
pub(crate) fn imported_names(body: &[Stmt]) -> Vec<(String, String, TextSize)> {
    let mut imported: Vec<(String, String, TextSize)> = Vec::new();
    for statement in body {
        let Stmt::ImportFrom(import) = statement else {
            continue;
        };
        if import.level != 0
            || import
                .module
                .as_ref()
                .is_none_or(|module| module.as_str() != ORCHESTRATION_MODULE)
        {
            continue;
        }
        for alias in &import.names {
            let local = alias.asname.as_ref().unwrap_or(&alias.name);
            imported.push((
                local.as_str().to_owned(),
                alias.name.as_str().to_owned(),
                alias.range().start(),
            ));
        }
    }
    imported
}

/// Which authoring call `call` is: an imported `calendar`, `sensor` or `run_event`, or one reached as an attribute.
pub(crate) fn authoring_call(
    call: &ExprCall,
    imported: &BTreeMap<&str, &str>,
) -> Option<&'static str> {
    let name: &str = match call.func.as_ref() {
        Expr::Name(name) => imported.get(name.id.as_str())?,
        Expr::Attribute(attribute) => attribute.attr.as_str(),
        _ => return None,
    };
    AUTHORING_CALLS.into_iter().find(|call| *call == name)
}

/// `expression` as a string literal argument, keeping where it starts whatever it is.
pub(crate) fn literal(expression: &Expr, lines: &SourceLines) -> LiteralArgument {
    LiteralArgument {
        value: match expression {
            Expr::StringLiteral(literal) => Some(literal.value.to_str().to_owned()),
            _ => None,
        },
        position: position(lines, expression.range().start()),
    }
}
