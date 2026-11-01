//! Schema values shared by fixture formatting and its object bridge, and prepared lint text.

use crate::lint_text::models::InterpolationSite;

/// A fixture column name, optional type, and optional nullability.
pub type FixtureFormatColumn = (String, Option<String>, Option<bool>);

/// Lint text with sentinels, its interpolation sites and the CTEs interpolation references.
pub type PreparedSql = (String, Vec<InterpolationSite>, Vec<String>);
