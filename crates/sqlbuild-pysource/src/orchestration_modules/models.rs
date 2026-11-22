//! The orchestration declarations a Python module makes, read without running it.

use std::collections::{BTreeMap, BTreeSet};

use ruff_python_ast::Expr;
use ruff_text_size::TextSize;

use crate::cpython_tree::models::SourceLines;

/// What a `calendar(...)` call or a `sensor(...)` decorator declares.
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

/// How a `sensor` declaration is written; only a call decorating a function declares one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SensorForm {
    /// `@sensor(...)` above `def function(...)`.
    Decorator { function: String },
    /// `@sensor` with no arguments.
    BareDecorator,
    /// `sensor(...)` that decorates no function.
    Call,
}

/// One calendar or sensor and the job names it refers to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PythonDeclaration {
    pub kind: PythonDeclarationKind,
    /// The `name` argument, else a decorated sensor's function name; `None` when there is neither.
    pub name: Option<LiteralArgument>,
    pub position: SourcePosition,
    /// Each `run_event(job=...)`, then each literal `RunRequest(job=...)` in a sensor's function, labelled by the call.
    pub job_references: Vec<(&'static str, LiteralArgument)>,
    /// A sensor's form; `None` for a calendar.
    pub sensor_form: Option<SensorForm>,
    /// Where a sensor's first positional argument starts, which it doesn't take.
    pub positional: Option<SourcePosition>,
    /// A sensor's `targets`: empty when omitted, `None` when not a literal tuple or list of strings.
    pub targets: Option<Vec<String>>,
    /// What a sensor's `when` selects, as far as its own module shows; `None` for a condition that selects no nodes.
    pub condition: Option<ConditionSource>,
}

/// Where a sensor condition's selection comes from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConditionSource {
    /// The `select` of a `node_event`, `source_new_data` or `source_stale` call; `None` when not a string literal.
    Selection(LiteralArgument),
    /// A name imported from `module`, `level` packages up for a relative import, to resolve there.
    Imported {
        module: String,
        level: u32,
        name: String,
        position: SourcePosition,
    },
    /// A value the reader cannot follow, described for the finding.
    Unresolvable {
        what: String,
        position: SourcePosition,
    },
}

/// The orchestration facts of one module.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ModuleDeclarations {
    pub declarations: Vec<PythonDeclaration>,
    /// Each removed authoring name the module imports from `sqlbuild.orchestration`.
    pub removed_imports: Vec<(String, SourcePosition)>,
}

/// The names a module's authoring calls resolve against: its imports, imported modules and module-level bindings.
#[derive(Debug)]
pub(crate) struct ModuleScope<'a> {
    pub(crate) imported: BTreeMap<&'a str, &'a str>,
    pub(crate) modules: BTreeSet<String>,
    /// Each module-level name's bindings, in source order.
    pub(crate) assignments: BTreeMap<&'a str, Vec<(TextSize, &'a Expr)>>,
    /// Each name's module-level `from ... import` bindings, in source order.
    pub(crate) imports: BTreeMap<&'a str, Vec<ImportedName<'a>>>,
    pub(crate) lines: &'a SourceLines,
    pub(crate) contents: &'a str,
}

/// A name a module-level `from ... import` binds.
#[derive(Debug)]
pub(crate) struct ImportedName<'a> {
    pub(crate) module: String,
    pub(crate) level: u32,
    pub(crate) name: &'a str,
    pub(crate) start: TextSize,
}
