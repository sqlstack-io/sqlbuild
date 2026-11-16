//! Merge project target config with local developer overrides.

use crate::contracts::_helpers::effective_config::effective_config_target_fields;
use crate::contracts::_helpers::spec_resolution::TARGET_GROUPS;
use crate::contracts::_helpers::spec_resolution::spec_error;
use pyo3::prelude::{Bound, PyAny, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::{PyDict, PyDictMethods, PyString};
use sqlbuild_python_base::records::main::construct::new_record::new_record;
use sqlbuild_python_base::records::types::RecordId;

/// Merge project target config with local developer overrides.
#[pyfunction]
#[pyo3(signature = (*, project_config, local_config, target_name))]
pub fn resolve_target_config<'py>(
    py: Python<'py>,
    project_config: &Bound<'py, PyAny>,
    local_config: &Bound<'py, PyAny>,
    target_name: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let (rows, error) = effective_config_target_fields(
        py,
        (project_config.clone(), local_config.clone()),
        target_name,
        &new_record(py, RecordId::TargetConfig, (), None)?,
    )?;
    let Some(rows) = rows else {
        let message: Bound<'py, PyString> = error.unwrap_or_else(|| PyString::new(py, ""));
        return Err(spec_error(py, message)?);
    };
    let values: Bound<'py, PyDict> = PyDict::new(py);
    let grouped: Vec<(&str, Bound<'py, PyDict>)> = TARGET_GROUPS
        .iter()
        .map(|(group, _)| (*group, PyDict::new(py)))
        .collect();
    for (group, name, value) in rows {
        match group {
            None => values.set_item(name, value)?,
            Some(group) => {
                if let Some((_, fields)) = grouped.iter().find(|(known, _)| *known == group) {
                    fields.set_item(name, value)?;
                }
            }
        }
    }
    for ((group, class_name), (_, fields)) in TARGET_GROUPS.iter().zip(&grouped) {
        let nested: Bound<'py, PyAny> = new_record(py, *class_name, (), Some(fields))?;
        values.set_item(*group, nested)?;
    }
    new_record(py, RecordId::TargetConfig, (), Some(&values))
}
