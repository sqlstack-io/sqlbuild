//! The note and help for turning SQL analysis on, or `None` when it is effectively on.

use crate::discovery::_helpers::failures::guidance::SQL_ANALYSIS_KEY;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_config::project::main::sql_analysis_off_guidance::sql_analysis_off_guidance as analysis_off;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// The note and help for turning SQL analysis on, or `None` when it is effectively on.
pub fn sql_analysis_guidance(
    discovered_inputs: &Bound<'_, PyAny>,
    purpose: &str,
) -> PyResult<Option<(String, String)>> {
    let local: Bound<'_, PyAny> = native_field(discovered_inputs, "local_config")?;
    let local_override: bool =
        native_field(&local, "setting_overrides")?.contains(SQL_ANALYSIS_KEY)?;
    let owner: Bound<'_, PyAny> = if local_override {
        local
    } else {
        native_field(discovered_inputs, "project_config")?
    };
    if native_field(&native_field(&owner, "settings")?, SQL_ANALYSIS_KEY)?.is_truthy()? {
        return Ok(None);
    }
    Ok(Some(analysis_off(local_override, purpose)))
}
