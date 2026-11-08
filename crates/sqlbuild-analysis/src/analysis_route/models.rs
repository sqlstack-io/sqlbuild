//! The route a dialect's analysis takes, and what stops a native parser reading SQL whole.

use sqlquack_semantics::lineage::models::{Nullability, SchemaColumn};
use sqlquack_semantics::lineage::types::LineageSchema;

use crate::analysis_route::types::NativeDialect;
use crate::assembly::analysis_session::models::{ColumnFact, LineageRow};
use crate::compiled_project::models::TextPosition;
use crate::semantic_validation::models::DiagnosticRow;

/// The engine that analyses a dialect's SQL.
#[derive(Clone, Copy)]
pub enum AnalysisRoute {
    /// Polyglot, as before the dialect's native module.
    Legacy,
    /// The dialect's native module; a model it cannot read whole is not analysed.
    Native(&'static dyn NativeDialect),
    /// No native module reads the dialect: nothing is analysed, and no Polyglot path stands in.
    Unavailable,
}

/// The first construct or error that stops a native parser, at its byte offset in the SQL it read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParserGap {
    /// The construct, such as `PIVOT`, or the parser's error message.
    pub construct: String,
    /// The parser's bare message, without the line and column an error's `Display` adds.
    pub message: String,
    pub offset: usize,
    /// Whether the parser reports invalid syntax; false for an unsupported construct or a limit.
    pub syntax: bool,
}

/// A gap with the line and column, from 1, of its offset in the analysed SQL.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LocatedGap {
    pub construct: String,
    pub message: String,
    pub line: usize,
    pub column: usize,
    pub syntax: bool,
    /// The gap's character offset in the SQL as checked, before normalisation, when its text holds.
    pub source_offset: Option<usize>,
}

/// Where P026 places a gap.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GapPosition {
    /// In the file the site names.
    File(TextPosition),
    /// In the site's own SQL statement, such as a hook's.
    Statement(TextPosition),
    /// Only in the analysed SQL, at the gap's own line and column.
    Analysed,
}

/// The SQL a gap stops: the hook it belongs to, if any, its file, and where the gap is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GapSite<'a> {
    /// Such as `post_hooks[0] sql("record_refresh")`; none for the model's query.
    pub label: Option<&'a str>,
    pub path: &'a str,
    pub position: GapPosition,
}

/// How an inferred column type meets its declared type: the type itself, then its nullability.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TypeMatch {
    /// One side's type is unknown or refused: no type finding, though null may still be forbidden.
    NotCompared {
        /// The inferred column may be null, but its declared type does not allow it.
        nullable_into_non_null: bool,
    },
    Compared {
        same_type: bool,
        /// The inferred column may be null, but its declared type does not allow it.
        nullable_into_non_null: bool,
    },
}

/// What a native module analyses: one query, the relations it may read, and how to name them.
#[derive(Clone, Copy, Debug)]
pub struct AnalysisInput<'a> {
    /// The analysis SQL, normalised with the dialect's stubs and its placeholders filled.
    pub sql: &'a str,
    /// The columns, types and nullability of the relations the query may read.
    pub schema: &'a dyn LineageSchema,
    /// `(analysis name, resource type, resource name)` of each relation the model references.
    pub references: &'a [(String, String, String)],
    /// Functions the catalogue cannot know, such as MotherDuck's `md_*`; never reported unknown.
    pub extension_functions: &'a [String],
    /// Whether quoted identifiers compare without case, as Snowflake's session parameter sets.
    pub quoted_ignore_case: bool,
}

/// One query's analysis, in the facts the analysis session keeps for a model.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct NativeAnalysis {
    /// The output columns in order, each typed through the dialect's `normalize_type`.
    pub columns: Vec<ColumnFact>,
    /// One row per output column: how it is derived and the referenced columns it reads.
    pub lineage: Vec<LineageRow>,
    /// `(relation, column)` of the columns that decide which rows the query returns.
    pub row_sources: Vec<(String, String)>,
    /// The outermost query level projects a star.
    pub has_star: bool,
    /// Every column a star projects is known.
    pub star_resolved: bool,
    /// The findings SQLBuild reports, in its codes and at character positions in `sql`.
    pub diagnostics: Vec<DiagnosticRow>,
}

/// One relation of known columns, standing in for what a probe query such as a macro's reads.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StandInSchema {
    relation: String,
    columns: Vec<SchemaColumn>,
}

impl StandInSchema {
    /// `relation` with each `(column, type as the dialect spells it, nullable)` in order.
    #[must_use]
    pub fn new(relation: &str, columns: &[(&str, &str, bool)]) -> Self {
        Self {
            relation: relation.to_owned(),
            columns: columns
                .iter()
                .map(|&(name, data_type, nullable)| SchemaColumn {
                    name: name.to_owned(),
                    quoted: false,
                    data_type: Some(data_type.to_owned()),
                    nullability: if nullable {
                        Nullability::Nullable
                    } else {
                        Nullability::NonNull
                    },
                })
                .collect(),
        }
    }
}

impl LineageSchema for StandInSchema {
    fn columns(&self, relation: &str) -> Option<&[SchemaColumn]> {
        relation
            .eq_ignore_ascii_case(&self.relation)
            .then_some(self.columns.as_slice())
    }
}

/// BigQuery, whose SQL SQLQuack's GoogleSQL parser reads.
#[derive(Clone, Copy, Debug)]
pub struct BigQueryDialect;

/// A query's schema that also knows each type BigQuery's type grammar reads, such as RANGE<DATE>.
#[derive(Debug)]
pub(crate) struct BigQueryTypes<'a> {
    pub(crate) schema: &'a dyn LineageSchema,
}

/// ClickHouse, whose SQL SQLQuack's ClickHouse parser reads.
#[derive(Clone, Copy, Debug)]
pub struct ClickHouseDialect;

/// DuckDB and MotherDuck, whose type spellings the native route compares as Polyglot did.
#[derive(Clone, Copy, Debug)]
pub struct DuckDbDialect;

/// PostgreSQL, whose SQL SQLQuack's PostgreSQL 18 parser reads.
#[derive(Clone, Copy, Debug)]
pub struct PostgresDialect;

/// Snowflake, whose SQL SQLQuack's Snowflake parser reads.
#[derive(Clone, Copy, Debug)]
pub struct SnowflakeDialect;

/// SQLite, whose SQL SQLQuack's native SQLite parser reads.
#[derive(Clone, Copy, Debug)]
pub struct SqliteDialect;

/// How a dialect's type names are respelt before SQLBuild's type fallback reads them.
#[derive(Clone, Copy, Debug)]
pub struct TypeSpelling {
    /// Each name, upper-cased and single-spaced, and the name it is compared as.
    pub aliases: &'static [(&'static str, &'static str)],
    /// Further aliases that apply only inside another type, such as a list's element.
    pub nested_aliases: &'static [(&'static str, &'static str)],
    /// Names whose parameters are dropped when they are renamed.
    pub sizeless: &'static [&'static str],
    /// Names whose parameterised spelling, as a whole type, the fallback cannot spell as compared.
    pub sizeless_whole: &'static [&'static str],
    /// Names kept as written when they have no parameters.
    pub bare_kept: &'static [&'static str],
    /// Types whose parameters are named fields.
    pub fielded: &'static [&'static str],
    /// Types whose parameters are compared as written.
    pub verbatim: &'static [&'static str],
    /// Whether `[]` array suffixes are part of the dialect's type syntax.
    pub arrays: bool,
}

/// SQL Server, whose SQL SQLQuack's T-SQL parser reads.
#[derive(Clone, Copy, Debug)]
pub struct TsqlDialect;
