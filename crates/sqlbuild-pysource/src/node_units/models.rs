//! The shape of a Python module under `python/`, as the node unit rules read it.

/// A 1-based line and character column.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct UnitPosition {
    pub line: usize,
    pub column: usize,
}

/// A module's top-level statements and every name it imports at module level.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct UnitModule {
    pub statements: Vec<UnitStatement>,
    pub imports: Vec<UnitImport>,
}

/// One top-level statement and where it starts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnitStatement {
    pub kind: UnitStatementKind,
    pub position: UnitPosition,
}

/// What a top-level statement is, as far as the unit rules care.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UnitStatementKind {
    /// The module docstring.
    Docstring,
    /// An import, or an `if TYPE_CHECKING:` block that only imports.
    Import,
    Function(UnitFunction),
    Class(UnitClass),
    /// An assignment, annotated or not, by the names it binds; empty when any target is an attribute or subscript.
    Assignment(Vec<String>),
    /// Any other statement, such as a loop or a call.
    Other,
}

/// A class and its bases.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnitClass {
    pub name: String,
    /// Where the class's name is written.
    pub position: UnitPosition,
    /// Each base as dotted text, such as `Provider` or `paged.PagedApi`; empty when it is another expression.
    pub bases: Vec<String>,
}

/// One imported name of an `import` or `from ... import` at module level.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnitImport {
    /// The dotted module after `level` leading dots; for `import a.b`, `a.b`.
    pub module: String,
    pub level: u32,
    /// The imported name of `from module import name`; `None` for `import module`.
    pub name: Option<String>,
    /// The name the import binds in the module.
    pub local: String,
    pub position: UnitPosition,
    /// Whether the bound name is used, and every use sits inside a decorator's `depends_on` value.
    pub only_in_depends_on: bool,
}

/// A function, its decorators, signature and body shape, and the decorated functions inside it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnitFunction {
    pub name: String,
    /// Where the function's name is written.
    pub position: UnitPosition,
    pub decorators: Vec<UnitDecorator>,
    /// Positional-only and ordinary parameters, in order.
    pub positional: Vec<UnitParameter>,
    /// Whether it takes `*args`.
    pub variadic: bool,
    pub keyword_only: Vec<UnitParameter>,
    /// Whether it takes `**kwargs`.
    pub keywords_variadic: bool,
    /// The last name of each member of the return annotation, or `None` without one.
    pub returns: Option<Vec<String>>,
    pub docstring: bool,
    /// Whether the body is exactly one `return` statement after any docstring.
    pub returns_directly: bool,
    /// For a body that is exactly `return main(...)` after any docstring, whether every argument is a named keyword.
    pub delegation: Option<bool>,
    /// The decorated functions defined anywhere in the body.
    pub inner: Vec<UnitFunction>,
    /// The names each `for` loop's iterable in the body loads, such as `enumerate` and `SPECS` for `enumerate(SPECS.items())`.
    pub loops: Vec<Vec<String>>,
}

/// A decorator by its last name, the keywords it is called with and the names its `depends_on` reads.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnitDecorator {
    pub name: String,
    pub keywords: Vec<String>,
    pub depends_on: Vec<String>,
}

/// A parameter and the last name of its annotation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnitParameter {
    pub name: String,
    pub annotation: Option<String>,
}

/// Source a module's import never runs, from `start` up to `end`, each a 1-based line and 0-based UTF-8 byte column.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DeferredSpan {
    pub start: (usize, usize),
    pub end: (usize, usize),
}

impl DeferredSpan {
    /// Whether the 1-based `line` and 0-based byte `column` fall inside the span.
    #[must_use]
    pub fn contains(&self, line: usize, column: usize) -> bool {
        self.start <= (line, column) && (line, column) < self.end
    }
}
