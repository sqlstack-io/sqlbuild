//! The providers a target expects to miss secrets.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use pyo3::types::{PyDict, PyDictMethods};

use sqlbuild_python_base::records::main::fields::field::field;

/// `[targets.<target>] allow_missing_secrets` from `ProjectConfig`, empty without the target.
pub fn target_allowed_missing_secrets(
    project_config: &Bound<'_, PyAny>,
    target: Option<&str>,
) -> PyResult<Vec<String>> {
    let Some(target) = target else {
        return Ok(Vec::new());
    };
    let targets = field(project_config, "targets")?;
    let Ok(targets) = targets.downcast::<PyDict>() else {
        return Ok(Vec::new());
    };
    let Some(config) = targets.get_item(target)? else {
        return Ok(Vec::new());
    };
    let allowed = field(&config, "allow_missing_secrets")?;
    if allowed.is_none() {
        return Ok(Vec::new());
    }
    allowed
        .try_iter()?
        .map(|name| name?.extract::<String>())
        .collect()
}
