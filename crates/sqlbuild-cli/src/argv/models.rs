//! The `sqb` command table, parsed values and the outcome of one argv parse.

use std::collections::HashMap;

use sqlbuild_core::dbt_cli::models::DbtParsedFlags;

/// How many command-line strings an action consumes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Nargs {
    /// `nargs=None`: exactly one string.
    Single,
    /// `nargs=0`: a flag.
    Zero,
    /// `nargs='?'`.
    Optional,
    /// `nargs='=?'`: an option whose value is given only as `--option=value`, never as the next string.
    Attached,
    /// `nargs='*'`.
    ZeroOrMore,
    /// `nargs='+'`.
    OneOrMore,
    /// `nargs='A...'`: a sub-command name and everything after it.
    Parser,
}

/// The `argparse` action class of one argument.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionKind {
    Store,
    StoreTrue,
    StoreFalse,
    StoreConst,
    Append,
    Extend,
    Help,
    Version,
    Parsers,
}

/// A Python value the parse stores, as the binding rebuilds it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Value {
    None,
    Bool(bool),
    /// A Python `int` as its canonical decimal text.
    Int(String),
    Str(String),
    /// A `pathlib.Path` built from this text.
    Path(String),
    List(Vec<Value>),
    /// An empty `dict`.
    EmptyDict,
    /// A value only the host can represent, by the handle it gave it.
    Host(usize),
}

/// An action default: a value, or `argparse.SUPPRESS`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Default {
    Suppress,
    Value(Value),
}

/// Help text: none, `argparse.SUPPRESS`, or text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HelpText {
    None,
    Suppress,
    Text(String),
}

/// One sub-command's help line in its parent's help.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChoiceHelp {
    pub metavar: String,
    pub help: HelpText,
}

/// One `add_argument` call of a parser.
#[derive(Debug, Clone)]
pub struct ActionSpec {
    pub id: usize,
    pub kind: ActionKind,
    pub option_strings: Vec<String>,
    /// The destination name; `None` for `argparse.SUPPRESS`.
    pub dest: Option<String>,
    pub nargs: Nargs,
    pub constant: Value,
    pub default: Default,
    /// The `type=` converter by name: `int`, `Path`, `_positive_integer` or `parse_cli_vars`.
    pub converter: Option<String>,
    pub choices: Option<Vec<String>>,
    pub required: bool,
    pub help: HelpText,
    /// The metavar: `None` when unset, `Some(None)` for `argparse.SUPPRESS`.
    pub metavar: Option<Option<String>>,
    pub group: usize,
    pub mutex_group: Option<usize>,
    pub version: Option<String>,
    /// Sub-command names and their parsers, in order.
    pub parsers: Vec<(String, ParserSpec)>,
    pub choices_help: Vec<ChoiceHelp>,
    /// Values no longer accepted, each with what to use instead; never listed in help.
    pub removed_choices: Vec<(String, String)>,
}

/// One argument group's title and description.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroupSpec {
    pub title: Option<String>,
    pub description: Option<String>,
}

/// One `ArgumentParser`: its texts, groups and actions in order.
#[derive(Debug, Clone)]
pub struct ParserSpec {
    pub prog: String,
    pub usage: Option<String>,
    pub description: Option<String>,
    pub epilog: Option<String>,
    pub groups: Vec<GroupSpec>,
    /// Whether each mutually exclusive group is required.
    pub mutex_groups: Vec<bool>,
    pub actions: Vec<ActionSpec>,
    /// Each option string's action, in `add_argument` order.
    pub option_actions: Vec<(String, usize)>,
    pub option_index: HashMap<String, usize>,
}

/// An `argparse.Namespace`: attributes in assignment order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Namespace {
    pub entries: Vec<(String, Value)>,
    /// `_UNRECOGNIZED_ARGS_ATTR`, set while a sub-command's leftovers travel up.
    pub unrecognized: Option<Vec<String>>,
}

/// Where one write of the parse goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stream {
    Stdout,
    Stderr,
}

/// What the caller decides about the parse's output.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArgvSettings {
    /// The terminal width `shutil.get_terminal_size().columns` reports.
    pub columns: i64,
    /// Whether the `error[C900]:` prefix is coloured.
    pub use_color: bool,
}

/// Why a `type=` converter rejected a string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeFailure {
    /// `argparse.ArgumentTypeError` with this message.
    ArgumentType(String),
    /// `TypeError` or `ValueError`: `invalid <type> value: '<text>'`.
    Invalid,
}

/// The `prog` of the parser that rejected the arguments, and its message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsageFailure {
    pub prog: String,
    pub message: String,
}

/// The writes of the parse, then either the namespace or the exit status.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArgvOutcome {
    pub writes: Vec<(Stream, String)>,
    pub parsed: Result<Namespace, i64>,
    /// The usage failure that ended the parse, if one did.
    pub usage_failure: Option<UsageFailure>,
}

/// Why a `sqb dbt` execution parse stopped before producing flags.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DbtExecutionFailure {
    /// `-h` printed the help: `SystemExit` with this status.
    Exit(i64),
    /// `ArgumentParser.error`: the parser's `prog`, then the `ArgumentError` text.
    Argument(String),
    /// Strings before `--` that no declared flag consumed.
    Unrecognized(Vec<String>),
}

/// The writes of a `sqb dbt` execution parse, then its flags or why it stopped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DbtExecutionOutcome {
    pub writes: Vec<(Stream, String)>,
    pub parsed: Result<DbtParsedFlags, DbtExecutionFailure>,
}

/// One `CliNamespace` attribute default, rebuilt fresh for every namespace.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NamespaceDefault {
    None,
    Bool(bool),
    Int(i64),
    Text(&'static str),
    /// A new empty `list`.
    EmptyList,
    /// A new empty `dict`.
    EmptyDict,
    /// `CompileLineageMode.FAST`.
    CompileLineageFast,
    /// `ColumnLineageMode.RICH`.
    ColumnLineageRich,
}
