//! How one parsed `sqb` invocation becomes a call of its Python command handler.

/// A class `sqlbuild._native` registers, by name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClassRef {
    pub name: &'static str,
}

/// Where one handler argument or request field takes its value from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    /// A parsed namespace attribute, as is.
    Attr(&'static str),
    /// `tuple(<attribute>)`.
    TupleOf(&'static str),
    /// `str(<attribute>)`.
    Text(&'static str),
    /// `<attribute> or <default>`.
    OrDefault(&'static str, &'static str),
    /// `None`, or `Path(<attribute>)`.
    OptionalPath(&'static str),
    /// A constant boolean.
    Flag(bool),
    /// `<class>(<attribute>)`: an enum member from a parsed value.
    Member(ClassRef, &'static str),
    /// `<class>(<value>)`: an enum member from a constant value.
    Constant(ClassRef, &'static str),
    /// `<class>(**fields)`.
    Nested(ClassRef, &'static [Field]),
    /// The resolved `--project-dir`, or `None`.
    ProjectDir,
    /// The resolved `--project-dir`, or the working directory.
    EffectiveProjectDir,
    /// `--select` followed by the selectors read from `--select-file`.
    Select,
    /// The selector files read, with how many selectors each held.
    SelectorFiles,
    /// `None` with `--no-limit`, else `--limit`.
    QueryLimit,
    /// One side of the parsed `diff` target range, `None` without a range.
    DiffName(DiffSide),
    /// The selection plus the `format` positional paths as selectors.
    FormatSelect,
    /// The scenario selectors followed by the selection.
    ScenarioSelect,
}

/// Which name of a `from..to` diff range.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffSide {
    From,
    To,
}

/// One keyword argument of a handler call or request class.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Field {
    pub name: &'static str,
    pub source: Source,
}

/// How the handler is called with its arguments.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CallShape {
    /// How many leading arguments an injected handler slot receives positionally.
    pub slot_positional: usize,
    /// Whether the command implementation receives every argument by keyword.
    pub implementation_keywords: bool,
}

/// One command's handler slot, implementation and arguments.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Route {
    /// The `CliEntrypointHandlers` attribute tests may inject.
    pub slot: &'static str,
    /// The name of the native function implementing the command.
    pub implementation: &'static str,
    pub arguments: &'static [Field],
    pub shape: CallShape,
    /// A keyword the implementation alone receives, such as the dbt command.
    pub implementation_extra: Option<Field>,
    /// The code an injected handler set without this slot fails with, if it may lack it.
    pub unavailable_code: Option<&'static str>,
}

/// A user error the dispatcher raises before calling a handler.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DispatchFailure {
    pub message: String,
    pub code: &'static str,
    pub help: Option<&'static str>,
}

/// The parsed values that select a route or reject the invocation before it runs.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CommandFacts {
    pub command: Option<String>,
    pub subcommand: Option<String>,
    pub json: bool,
    pub from_target: bool,
    pub contract_ready: bool,
    pub format_diff: bool,
    pub format_fix: bool,
    pub format_fixtures_only: bool,
    pub scenario_local: bool,
    pub scenario_retain: bool,
    pub scenario_snapshot_sync: bool,
    /// Whether `diff` compares raw queries rather than project models.
    pub raw_query_diff: bool,
    /// Whether `--ignore-audit-failures` is combined with `--no-audits`, which leaves nothing to ignore.
    pub ignores_failures_of_no_audits: bool,
}

/// Which invocation services run around a command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvocationPolicy {
    /// `init`, `playground` and `dbt init` create a project, so nothing reads one first.
    pub creates_project: bool,
    /// Whether invocation lifecycle events and progress are published.
    pub observed: bool,
    /// Whether installed SQLBuild skills are checked for freshness first.
    pub maintains_skills: bool,
    /// Whether Recall records the invocation; `sqb recall` never records itself.
    pub recalled: bool,
    /// Whether compute logs capture the process streams; read-only `sqb recall` skips them.
    pub captured: bool,
}

/// What a failed command reports: its fallback error code and the debug log message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandFailure {
    /// `CliUserError` and `RulesError`.
    User,
    /// `LintError`.
    Lint,
    /// Expected discovery, configuration, audit and execution-limit failures.
    Command,
}

/// The route of a command, or the failure that stops it, or nothing to run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Dispatch {
    /// The route, and the failure raised once an injected handler is known to exist.
    Run(&'static Route, Option<DispatchFailure>),
    Fail(DispatchFailure),
    /// Commands with no handler exit successfully.
    Nothing,
}

/// Lifecycle event export accounting of one sink at command exit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExporterLoss {
    pub exporter_name: String,
    pub dropped: u64,
    pub failed: u64,
}

/// Lifecycle event export accounting of every sink at command exit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportSummary {
    pub accepted: u64,
    pub dropped: u64,
    pub failed: u64,
    pub per_exporter: Vec<ExporterLoss>,
    /// Each sink provider whose teardown raised, with its error type.
    pub teardown_failures: Vec<String>,
}
