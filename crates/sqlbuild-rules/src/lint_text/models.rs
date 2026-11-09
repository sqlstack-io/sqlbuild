//! Interpolation sites, suppression outcomes and fixture-null candidates.

use crate::lint_text::types::FixtureFormatColumn;

/// A schema whose names are authoritative for an authored fixture CTE.
#[derive(Clone, Debug)]
pub struct FixtureFormatRelation {
    pub columns: Vec<FixtureFormatColumn>,
}

/// An authored SQL-test block eligible for model fixture formatting.
#[derive(Clone, Debug)]
pub struct FixtureFormatBlock {
    pub sql_body: String,
    pub model_mode: bool,
}

/// Dependency sentinels and keyed relations derived from authored interpolation sites.
#[derive(Debug, Default)]
pub struct DependencySites {
    pub identifiers: Vec<String>,
    pub relations: Vec<(String, String)>,
}

/// One interpolation occurrence replaced by a sentinel; offsets are character positions.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InterpolationSite {
    pub sentinel: String,
    pub neutralized_start: usize,
    pub neutralized_end: usize,
    pub original_start: usize,
    pub original_end: usize,
    pub original_text: String,
}

/// Text with its interpolation replaced by sentinels, and the replaced sites in order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Neutralized {
    pub text: String,
    pub sites: Vec<InterpolationSite>,
}

/// The rejection finding on one inline directive line; offsets are character positions.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SuppressionFinding {
    pub file: usize,
    pub line: usize,
    pub message: String,
    pub remediation: String,
    pub fault: bool,
    pub fix: Option<(usize, usize)>,
    pub fix_unavailable_reason: Option<&'static str>,
}

/// SQL-test files that may hold removable typed nulls, and the models their fixtures stand in for.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FixtureNullCandidates {
    pub paths: Vec<String>,
    pub model_names: Vec<String>,
}

/// One SQL file to format: its project-relative path when inside the project, and its text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FormatFile {
    pub relative_parts: Option<Vec<String>>,
    pub contents: String,
}

/// A body the formatter declined, by file index, one-based line and reason.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FormatFault {
    pub file: usize,
    pub line: usize,
    pub reason: String,
    /// The dialect has no formatter yet: the file is left as written with a warning, not a fault.
    pub unavailable: bool,
}

/// Formatted file texts by file index, and the bodies left unchanged.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FormattedBodies {
    pub formatted: Vec<(usize, String)>,
    pub faults: Vec<FormatFault>,
}

/// One authored body prepared for batched formatting; offsets are character positions.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PreparedBody {
    pub(crate) start: usize,
    pub(crate) end: usize,
    pub(crate) trailing: String,
    pub(crate) request: usize,
    pub(crate) sites: Vec<InterpolationSite>,
    pub(crate) omits_select: bool,
    /// The body is a SQL macro or function expression, formatted as the one item of a `SELECT`.
    pub(crate) wraps_expression: bool,
}

/// One substitution of an expansion pass: authored source range and generated output range.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct ExpansionSpan {
    pub source_start: i64,
    pub source_end: i64,
    pub output_start: i64,
    pub output_end: i64,
}

/// One expanded, neutralized authored body to lint; offsets are character positions.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LintSqlBody {
    pub file: usize,
    pub body_start: i64,
    pub lint_text: String,
    pub passes: Vec<Vec<ExpansionSpan>>,
    pub external_identifiers: Vec<String>,
    pub dependency_identifiers: Vec<String>,
    pub externally_referenced_ctes: Vec<String>,
    pub allows_ceremonial_select: bool,
    pub allows_dynamic_output_star: bool,
    pub allows_empty_fixture_star: bool,
    pub relation_keys: Vec<(String, Vec<Vec<String>>)>,
    /// The relation name each dependency sentinel stands for.
    pub relation_names: Vec<(String, String)>,
    /// The generated characters before and after an expression macro body framed as a `SELECT`.
    pub expression_frame: Option<(i64, i64)>,
}

/// An expression macro body framed as a `SELECT`, the pass back to the body, and the frame size.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FramedExpression {
    pub text: String,
    pub pass: Vec<ExpansionSpan>,
    /// The generated characters before the expression and after it, to the end of `text`.
    pub frame: (i64, i64),
}

/// The rule selection and thresholds of one SQL lint run.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LintSqlOptions {
    pub dialect: String,
    pub enabled_rules: Option<Vec<String>>,
    pub ignored_rules: Vec<String>,
    pub max_literal_length: i64,
    pub max_ranking_order_by: i64,
}

/// An authored-source replacement proposed by a rule; offsets are character positions.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuthoredEdit {
    pub start: i64,
    pub end: i64,
    pub replacement: String,
}

/// One SQL lint diagnostic mapped onto authored source.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuthoredDiagnostic {
    pub file: usize,
    pub line: i64,
    pub column: i64,
    pub code: String,
    pub message: String,
    pub fault: bool,
    pub end: Option<(i64, i64)>,
    pub remediation: String,
    pub fix: Option<AuthoredEdit>,
    pub fix_unavailable_reason: Option<String>,
}

/// Format and lint settings resolved from the project and local configuration files.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FormatSettings {
    pub line_width: i64,
    pub max_description_lines: i64,
    pub dialect: String,
    pub max_literal_length: i64,
    pub max_ranking_order_by: i64,
    /// Whether the project configuration file exists, so its rules configuration applies.
    pub has_project_config: bool,
}
