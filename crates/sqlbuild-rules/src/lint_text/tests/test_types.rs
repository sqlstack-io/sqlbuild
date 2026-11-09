use crate::lint_text::models::{Neutralized, SuppressionFinding};

pub(crate) struct NeutralizeTestCase {
    pub(crate) description: &'static str,
    pub(crate) body: &'static str,
    pub(crate) formatting: bool,
    pub(crate) expected_text: &'static str,
    pub(crate) expected_sites: Vec<(&'static str, usize, usize)>,
}

pub(crate) struct SlotReadTestCase {
    pub(crate) description: &'static str,
    pub(crate) body: &'static str,
    pub(crate) expected_text: &'static str,
}

pub(crate) struct ParameterTestCase {
    pub(crate) description: &'static str,
    pub(crate) body: &'static str,
    pub(crate) neutralize: fn(&str) -> Neutralized,
    pub(crate) expected_text: &'static str,
}

pub(crate) struct RestoreTestCase {
    pub(crate) description: &'static str,
    pub(crate) body: &'static str,
    pub(crate) fixed: &'static str,
    pub(crate) expected_restored: Result<String, String>,
}

pub(crate) struct TokenAtTestCase {
    pub(crate) description: &'static str,
    pub(crate) body: &'static str,
    pub(crate) start: usize,
    pub(crate) expected_token: Option<&'static str>,
}

pub(crate) struct SuppressionTestCase {
    pub(crate) description: &'static str,
    pub(crate) contents: &'static str,
    pub(crate) expected_findings: Vec<SuppressionFinding>,
}

pub(crate) struct FormatBodiesTestCase {
    pub(crate) description: &'static str,
    pub(crate) dialect: &'static str,
    pub(crate) relative_parts: Option<Vec<String>>,
    pub(crate) contents: &'static str,
    pub(crate) expected_formatted: Vec<(usize, String)>,
    pub(crate) expected_fault_lines: Vec<usize>,
}

pub(crate) struct LintBodiesTestCase {
    pub(crate) description: &'static str,
    pub(crate) contents: &'static str,
    pub(crate) body: crate::lint_text::models::LintSqlBody,
    pub(crate) expected_findings: Vec<Finding<'static>>,
}

pub(crate) struct ExitCountTestCase {
    pub(crate) description: &'static str,
    pub(crate) code: &'static str,
    pub(crate) expected_counts: bool,
}

pub(crate) struct UnavailableLintTestCase {
    pub(crate) description: &'static str,
    pub(crate) response: serde_json::Value,
    pub(crate) bodies_in_file: usize,
    pub(crate) expected_findings: Vec<UnavailableFinding<'static>>,
}

/// One authored finding: code, line, column, whether it is a fault, and its message.
pub(crate) type UnavailableFinding<'a> = (&'a str, i64, i64, bool, &'a str);

/// One mapped finding: code, line, column and optional end position.
pub(crate) type Finding<'a> = (&'a str, i64, i64, Option<(i64, i64)>);

pub(super) struct FormatSettingsTestCase {
    pub(super) description: &'static str,
    pub(super) project_toml: &'static str,
    pub(super) local_toml: &'static str,
    pub(super) expected_settings: crate::lint_text::models::FormatSettings,
}

pub(super) struct FirstHeaderTestCase {
    pub(super) description: &'static str,
    pub(super) relative_parts: &'static [&'static str],
    pub(super) expected_first_only: bool,
}

pub(super) struct RoleKindTestCase {
    pub(super) description: &'static str,
    pub(super) relative_parts: &'static [&'static str],
    pub(super) expected_kind: Option<&'static str>,
}
pub(crate) struct FixtureFormattingTestCase {
    pub description: &'static str,
    pub sql: &'static str,
    pub columns: Vec<(String, Option<String>, Option<bool>)>,
    pub expected_sql: &'static str,
}
/// Authored block formatting across fixture roles and ambiguous replacements.
pub(super) struct FixtureBlockTestCase {
    pub description: &'static str,
    pub sql: &'static str,
    pub model_mode: bool,
    pub expected_sql: Option<&'static str>,
}

pub(crate) struct SitePatternTestCase {
    pub(crate) description: &'static str,
    pub(crate) formatting: bool,
    pub(crate) expected_other_start_bytes: Vec<u8>,
}

pub(crate) struct UnwrappedExpressionTestCase {
    pub(crate) description: &'static str,
    pub(crate) formatted: &'static str,
    pub(crate) expected_expression: Option<&'static str>,
}
