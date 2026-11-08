//! A parsed SQL macro, its call arguments and what one expansion produced.

use crate::sql_macros::types::SqlMacroScope;

/// How a SQL macro's body starts, which decides where a call may stand and what it inserts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SqlMacroForm {
    /// Any SQL expression; the call stands in any expression position.
    Expression,
    /// A `WITH` CTE list, inserted flat into the caller's `WITH` list.
    CteList,
    /// A `SELECT` column list, inserted into the caller's select list.
    ColumnList,
}

impl SqlMacroForm {
    /// The form as error messages name it.
    pub fn label(self) -> &'static str {
        match self {
            Self::Expression => "expression",
            Self::CteList => "CTE-list",
            Self::ColumnList => "column-list",
        }
    }
}

/// What one parameter accepts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ParameterKind {
    /// Any SQL expression of a compatible type.
    Expression { data_type: String },
    /// Any scalar SQL expression, whatever its type, or any of one type family.
    Any { family: Option<AnyFamily> },
    /// A literal or a scalar constant.
    Constant { data_type: String },
    /// An inline list or set, or a list or set constant; `set` for `SET`.
    Collection { set: bool, data_type: String },
    /// A member of the named enum.
    Enum { name: String },
    /// A relation and its slots: single typed columns and `COLUMNS` groups the caller chooses.
    Relation {
        columns: Vec<(String, String)>,
        groups: Vec<String>,
    },
}

impl ParameterKind {
    /// The declared type as messages show it, such as `BIGINT` or `ENUM order_status`.
    pub fn label(&self) -> String {
        match self {
            Self::Expression { data_type } => data_type.clone(),
            Self::Any { family: None } => "ANY".to_owned(),
            Self::Any {
                family: Some(family),
            } => format!("ANY {}", family.keyword()),
            Self::Constant { data_type } => format!("{data_type} CONST"),
            Self::Collection { set, data_type } => {
                format!("{} {data_type}", if *set { "SET" } else { "LIST" })
            }
            Self::Enum { name } => format!("ENUM {name}"),
            Self::Relation { columns, groups } => {
                let listed: Vec<String> = columns
                    .iter()
                    .map(|(name, data_type)| format!("{name} {data_type}"))
                    .chain(groups.iter().map(|group| format!("{group} COLUMNS")))
                    .collect();
                format!("RELATION ({})", listed.join(", "))
            }
        }
    }

    /// Whether this is a `RELATION` that declares a `COLUMNS` slot called `slot`.
    pub fn declares_group(&self, slot: &str) -> bool {
        let Self::Relation { groups, .. } = self else {
            return false;
        };
        groups.iter().any(|group| group.eq_ignore_ascii_case(slot))
    }

    /// Whether this is a `RELATION`.
    pub fn declares_relation(&self) -> bool {
        matches!(self, Self::Relation { .. })
    }

    /// Whether this is a `RELATION` that declares a slot called `slot`.
    pub fn declares_slot(&self, slot: &str) -> bool {
        let Self::Relation { columns, groups } = self else {
            return false;
        };
        groups.iter().any(|group| group.eq_ignore_ascii_case(slot))
            || columns
                .iter()
                .any(|(column, _)| column.eq_ignore_ascii_case(slot))
    }
}

/// The caller's columns one slot of a `RELATION` argument reads, as the call mapped them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SlotMapping {
    /// The slot's name as the macro declares it.
    pub slot: String,
    /// The caller's columns: exactly one for a single-column slot, one or more for a group.
    pub columns: Vec<String>,
    /// Whether the slot is a `COLUMNS` group.
    pub group: bool,
}

/// A type family an `ANY` parameter may be narrowed to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnyFamily {
    /// Integer, decimal and float types.
    Numeric,
    /// String types.
    Text,
    /// Date, time and timestamp types.
    Temporal,
}

impl AnyFamily {
    /// Every family, in the order messages list them.
    pub const ALL: [Self; 3] = [Self::Numeric, Self::Text, Self::Temporal];

    /// The word after `ANY` that declares the family.
    pub fn keyword(self) -> &'static str {
        match self {
            Self::Numeric => "NUMERIC",
            Self::Text => "TEXT",
            Self::Temporal => "TEMPORAL",
        }
    }

    /// The family's widest member, which a macro body is type-checked against.
    pub fn widest(self) -> &'static str {
        match self {
            Self::Numeric => "DOUBLE",
            Self::Text => "VARCHAR",
            Self::Temporal => "TIMESTAMP",
        }
    }

    /// What the family's values are, as messages name them.
    pub fn noun(self) -> &'static str {
        match self {
            Self::Numeric => "a number",
            Self::Text => "text",
            Self::Temporal => "a date, time or timestamp",
        }
    }

    /// Whether a value of `family` belongs to this family; unknown and NULL values do.
    pub(crate) fn admits(self, family: TypeFamily) -> bool {
        matches!(
            (self, family),
            (_, TypeFamily::Unknown | TypeFamily::Null)
                | (
                    Self::Numeric,
                    TypeFamily::Integer | TypeFamily::Decimal | TypeFamily::Float
                )
                | (Self::Text, TypeFamily::Text)
                | (
                    Self::Temporal,
                    TypeFamily::Date | TypeFamily::Timestamp | TypeFamily::Time
                )
        )
    }
}

/// One declared parameter.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SqlMacroParameter {
    pub name: String,
    pub kind: ParameterKind,
    /// The `DEFAULT` text as written, resolved in the macro file's scope.
    pub default: Option<String>,
    /// One-based line and column of the parameter's name in the macro file.
    pub line: usize,
    pub column: usize,
}

/// One SQL macro file, parsed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SqlMacroDefinition {
    pub name: String,
    /// The macro file's project-relative path.
    pub path: String,
    pub description: Option<String>,
    pub parameters: Vec<SqlMacroParameter>,
    /// The declared result type of an expression macro.
    pub returns: Option<String>,
    /// The parameter whose argument's type an expression macro returns, by `returns TYPE OF`.
    pub returns_like: Option<String>,
    pub form: SqlMacroForm,
    /// The body as written, after the header.
    pub body: String,
    /// One-based line of the body's first character in the macro file.
    pub body_line: usize,
    /// One-based column of the body's first character in the macro file.
    pub body_column: usize,
    /// Every SQL macro the body calls, once each, in order.
    pub calls: Vec<String>,
}

impl SqlMacroDefinition {
    /// The declared parameter called `name`, if any.
    pub fn parameter(&self, name: &str) -> Option<&SqlMacroParameter> {
        self.parameters
            .iter()
            .find(|parameter| parameter.name.eq_ignore_ascii_case(name))
    }
}

/// One file location an error names.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SqlMacroLocation {
    pub path: String,
    pub line: usize,
    pub column: usize,
}

/// One argument of a call as written: its name if passed by name, its text and where it starts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CallArgument {
    pub name: Option<String>,
    /// The slot a `relation.slot := ...` argument maps; `name` is then the relation.
    pub slot: Option<String>,
    pub text: String,
    /// Byte offset of the argument text in the call's argument list.
    pub offset: usize,
}

/// One parameter's argument after binding: the call's text or the parameter's default.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BoundArgument {
    /// Index of the parameter in the definition.
    pub parameter: usize,
    pub text: String,
    /// The argument came from the parameter's `DEFAULT`, so it resolves in the macro's scope.
    pub from_default: bool,
    /// Byte offset of the argument in the call's argument list, when the call passed it.
    pub offset: Option<usize>,
    /// The slots a `RELATION` argument's call maps, in the order the macro declares them.
    pub slots: Vec<SlotMapping>,
}

/// The shape a typed argument was written in, which decides how it is checked and rendered.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ArgumentShape {
    /// Any SQL expression.
    Expression,
    /// A SQL literal, such as `2` or `'GB'`.
    Literal,
    /// `@const("name")`.
    ConstantReference { name: String },
    /// `@enum("name").MEMBER`.
    EnumMember { name: String, member: String },
    /// An inline `[...]` list or `{...}` set in constant syntax.
    InlineCollection,
    /// A plain identifier: a CTE name, a table alias or a column name.
    Identifier { name: String },
    /// `__ref("name")`, `__source("name")` or `__seed("name")`.
    Reference { call: String },
    /// `@name`: a parameter of the SQL macro whose body makes the call, passed on.
    ParameterReference { name: String },
}

/// A parameter's argument ready to substitute: its SQL and how it was written.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RenderedArgument {
    pub parameter: usize,
    pub sql: String,
    pub shape: ArgumentShape,
    /// The expression macro calls `sql` holds.
    pub calls: Vec<TypedCall>,
    /// The slots a `RELATION` argument's call maps.
    pub slots: Vec<SlotMapping>,
}

/// One expansion's SQL and the names it defines, for clash checks at the call site.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BodyExpansion {
    /// The SQL inserted at the call, without the body's leading `WITH` or `SELECT`.
    pub sql: String,
    /// The CTE names a CTE-list macro defines, or the column names a column-list macro outputs.
    pub names: Vec<String>,
    /// The expression macro calls the SQL holds, from the body and the arguments.
    pub calls: Vec<TypedCall>,
    /// The SQL of the argument whose type an expression macro call returns, by `returns TYPE OF`.
    pub like: Option<String>,
    /// Each plain `COLUMNS` slot read in a subquery, and the relations that could capture it.
    pub captures: Vec<SlotCapture>,
}

/// A plain `COLUMNS` slot read inside a subquery, whose bare columns could bind to its relations.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SlotCapture {
    /// The `RELATION` parameter and the group read.
    pub relation: String,
    pub slot: String,
    /// The group's columns at the call.
    pub columns: Vec<String>,
    /// Each relation of the enclosing subqueries, innermost first; `None` for an unnamed one.
    pub sources: Vec<Option<String>>,
}

/// One expression macro call's expansion in a text: its byte range and the macro's `returns`.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct TypedCall {
    pub start: usize,
    pub end: usize,
    pub macro_name: String,
    /// The macro's declared result type; `None` when it declares none.
    pub returns: Option<String>,
    /// The SQL of the argument whose type the call returns, by `returns TYPE OF`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub returns_like: Option<String>,
}

/// SQL an expansion produced and the expression macro calls it holds.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ExpandedText {
    pub sql: String,
    /// Each expression macro call in `sql`, inner calls before the calls holding them.
    pub calls: Vec<TypedCall>,
}

/// A replaced byte range of a text, and the byte range its replacement fills in the output.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TextSplice {
    pub source: (usize, usize),
    pub output: (usize, usize),
}

/// A text with its `@const` and `@enum` references rendered, and where each one landed.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RenderedDeclarations {
    pub sql: String,
    /// The rendered references in text order.
    pub splices: Vec<TextSplice>,
}

/// Where in the caller a call stands, as the text around it shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CallPosition {
    /// Inside a `WITH` list, where a CTE may stand.
    CteList,
    /// Inside a select list, where a column may stand.
    SelectList,
    /// Anywhere else.
    Expression,
}

/// A call's expansion placed into the caller: replacement text and the separators it needs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlacedExpansion {
    pub text: String,
}

/// One SQL macro call in a text: its name and byte spans.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SqlMacroCall {
    pub name: String,
    pub start: usize,
    pub end: usize,
    /// Where the text the expansion replaces ends: past a comma right after a list call.
    pub replaced_end: usize,
    pub arguments: (usize, usize),
    /// The call stands inside a Python macro call's arguments, so it expands to a string.
    pub in_python: bool,
}

/// Names inserted so far into each `WITH` or select list of one text, by the list's keyword.
#[derive(Clone, Debug, Default)]
pub struct InsertedNames {
    pub(crate) lists: Vec<(usize, String, String)>,
}

/// What one text's expansion reads: its text, scope, the macros being expanded and its label.
#[derive(Debug)]
pub struct TextContext<'a> {
    pub text: &'a str,
    pub scope: &'a SqlMacroScope,
    /// The SQL macros whose bodies are being expanded, outermost first.
    pub stack: &'a [String],
    /// How a clash names the text's owner, such as `model:orders_clean`.
    pub owner: &'a str,
}

/// Where a declaration file publishes: project-wide, to an owner and below, or to the owner only.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Placement {
    Project,
    Inherited(String),
    Local(String),
}

/// A broad SQL type family; `Unknown` fits everything.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TypeFamily {
    Integer,
    Decimal,
    Float,
    Text,
    Boolean,
    Date,
    Timestamp,
    Time,
    Null,
    Unknown,
}

/// What the caller's analysed SQL must show for one call's arguments to fit their types.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SqlMacroCheck {
    pub macro_name: String,
    /// Where the call stands among the caller's top-level query parts.
    pub scope: CallScope,
    /// Whether the call stands in its scope's one select, outside subqueries and set operations.
    pub plain_select: bool,
    pub arguments: Vec<ArgumentCheck>,
    /// The call's authored file location and where its text ends, when the host knows them.
    pub call: Option<(SqlMacroLocation, (usize, usize))>,
}

/// The top-level part of the caller's query a call stands in.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CallScope {
    /// Between the CTEs of the top-level `WITH` list.
    WithList,
    /// Inside the body of the named top-level CTE.
    Cte(String),
    /// In the query after the `WITH` list, or a query without one.
    Main,
}

/// One argument whose fit the caller's column types decide.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArgumentCheck {
    pub parameter: String,
    /// The argument as the caller wrote it, its declarations rendered.
    pub text: String,
    pub expected: ArgumentExpectation,
    /// The parameter's declaration in the macro file.
    pub declared_at: SqlMacroLocation,
}

/// What an argument must be.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ArgumentExpectation {
    /// A relation with these columns, each of a compatible type, and its mapped slots' columns.
    Relation {
        columns: Vec<(String, String)>,
        slots: Vec<SlotMapping>,
    },
    /// An expression of a type compatible with this one.
    Typed { data_type: String },
    /// An expression of a type in this family.
    Family { family: AnyFamily },
    /// A relation, `None` when unnamed, that must hold none of a plain group read's columns.
    Capture {
        slot: String,
        columns: Vec<String>,
        source: Option<String>,
    },
}

/// A call argument the caller's analysed SQL shows does not fit its parameter.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SqlMacroFinding {
    pub message: String,
    pub help: Option<String>,
    pub call: SqlMacroLocation,
    pub call_end: (usize, usize),
    pub declared_at: SqlMacroLocation,
}

/// The byte span of one parameter reference a SQL macro body's lint replaces.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParameterSpan {
    pub start: usize,
    pub end: usize,
    /// The `RELATION` parameter this span names as a relation, as `@weekly` in `FROM @weekly`.
    pub relation: Option<String>,
}

/// What type inference knows of an expression macro body over its declared arguments.
#[derive(Clone, Debug)]
pub(crate) enum BodyType {
    /// A completely known type.
    Known(String),
    /// Nothing, or only a guess.
    Unknown,
}
