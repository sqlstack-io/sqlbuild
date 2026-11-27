//! A load's settings as SQLLoad reads them: its own keys, with its sources' declared column types.

use crate::loads::_helpers::settings::with_source_columns;
use crate::loads::models::{LoadDeclaration, LoadProblem};

/// `load`'s settings with the column types of the sources it fills, as SQLLoad reads a source.
pub fn load_settings(load: &LoadDeclaration) -> Result<toml::Table, LoadProblem> {
    with_source_columns(load)
}
