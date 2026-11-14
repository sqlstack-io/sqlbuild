//! The orchestration declarations a Python module makes, read without running it.

/// What a module-level `calendar(...)` or `sensor(...)` call declares.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PythonDeclarationKind {
    Calendar,
    Sensor,
}

/// A 1-based line and column in the module source.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SourcePosition {
    pub line: usize,
    pub column: usize,
}

/// A string argument of a declaration; `None` when it is not a string literal.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LiteralArgument {
    pub value: Option<String>,
    pub position: SourcePosition,
}

/// One calendar or sensor and the job names it refers to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PythonDeclaration {
    pub kind: PythonDeclarationKind,
    /// The `name` argument, or `None` when the call passes no name.
    pub name: Option<LiteralArgument>,
    pub position: SourcePosition,
    /// `run=` of a sensor, then each `run_event(job=...)`, labelled by the argument.
    pub job_references: Vec<(&'static str, LiteralArgument)>,
    /// A sensor's `targets`: empty when omitted, `None` when not a literal tuple or list of strings.
    pub targets: Option<Vec<String>>,
}

/// The orchestration facts of one module.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ModuleDeclarations {
    pub declarations: Vec<PythonDeclaration>,
    /// Each removed authoring name the module imports from `sqlbuild.orchestration`.
    pub removed_imports: Vec<(String, SourcePosition)>,
}
