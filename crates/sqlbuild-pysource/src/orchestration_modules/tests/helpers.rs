use crate::orchestration_modules::models::{
    LiteralArgument, PythonDeclaration, PythonDeclarationKind, SourcePosition,
};

/// A 1-based line and column.
pub(super) fn at(line: usize, column: usize) -> SourcePosition {
    SourcePosition { line, column }
}

/// A string literal argument written at `line` and `column`.
pub(super) fn text(value: &str, line: usize, column: usize) -> LiteralArgument {
    LiteralArgument {
        value: Some(value.to_owned()),
        position: at(line, column),
    }
}

/// An argument that is not a string literal, written at `line` and `column`.
pub(super) fn non_literal(line: usize, column: usize) -> LiteralArgument {
    LiteralArgument {
        value: None,
        position: at(line, column),
    }
}

/// A declaration whose call starts at `position` and passes no `targets`.
pub(super) fn declaration(
    kind: PythonDeclarationKind,
    name: LiteralArgument,
    position: SourcePosition,
    job_references: Vec<(&'static str, LiteralArgument)>,
) -> PythonDeclaration {
    PythonDeclaration {
        kind,
        name: Some(name),
        position,
        job_references,
        targets: Some(Vec::new()),
    }
}
