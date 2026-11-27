//! The loads a plan runs: a selected load loads every table, else only its selected sources'.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyBool, PyString, PyTuple};
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::main::fields::mapping_get::mapping_get;
use sqlbuild_python_base::records::types::RecordId;

/// A source key's resource type.
const SOURCE_RESOURCE: &str = "source";

/// The loads a plan runs, and the keys of those it runs only because a source they fill is selected.
pub(crate) type PlannedLoads<'py> = (Vec<Bound<'py, PyAny>>, Vec<Bound<'py, PyAny>>);

/// A `LoadPlanEntry` for each load `selected_keys` names, or whose sources it selects, in project order.
pub(crate) fn planned_loads<'py>(
    project: &Bound<'py, PyAny>,
    selected_keys: &Bound<'py, PyAny>,
    source_map: &Bound<'py, PyAny>,
    reload: bool,
) -> PyResult<PlannedLoads<'py>> {
    let py: Python<'py> = project.py();
    let mut selected_sources: Vec<(String, Option<String>)> = Vec::new();
    for key in selected_keys.try_iter()? {
        let key: Bound<'py, PyAny> = key?;
        if native_field(&key, "resource_type")?
            .str()?
            .extract::<String>()?
            != SOURCE_RESOURCE
        {
            continue;
        }
        let name: Bound<'py, PyAny> = native_field(&key, "name")?;
        let entry: Bound<'py, PyAny> = mapping_get(source_map, &name)?;
        let load: Option<String> = if entry.is_none() {
            None
        } else {
            native_field(&entry, "load")?.extract()?
        };
        selected_sources.push((name.extract()?, load));
    }
    let (mut entries, mut added_keys): PlannedLoads<'py> = (Vec::new(), Vec::new());
    for load in native_field(project, "loads")?.try_iter()? {
        let load: Bound<'py, PyAny> = load?;
        let key: Bound<'py, PyAny> = native_field(&load, "key")?;
        let name: String = native_field(&load, "name")?.extract()?;
        let every_table: bool = selected_keys.contains(&key)?;
        let mut sources: Vec<&str> = selected_sources
            .iter()
            .filter(|(_, filled_by)| filled_by.as_deref() == Some(name.as_str()))
            .map(|(source, _)| source.as_str())
            .collect();
        if !every_table && sources.is_empty() {
            continue;
        }
        sources.sort_unstable();
        if !every_table {
            added_keys.push(key.clone());
        }
        entries.push(new_record_with(
            py,
            RecordId::LoadPlanEntry,
            &[
                ("key", key),
                ("name", PyString::new(py, &name).into_any()),
                ("sources", PyTuple::new(py, sources)?.into_any()),
                (
                    "every_table",
                    PyBool::new(py, every_table).to_owned().into_any(),
                ),
                ("fingerprint", native_field(&load, "fingerprint")?),
                ("is_reload", PyBool::new(py, reload).to_owned().into_any()),
            ],
        )?);
    }
    Ok((entries, added_keys))
}
