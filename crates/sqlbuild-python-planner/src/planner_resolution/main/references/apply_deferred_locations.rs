//! Apply deferred locations.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use pyo3::pyfunction;
use pyo3::types::{PyDict, PyDictMethods};
use sqlbuild_python_base::records::main::fields::field::field as native_field;

#[pyfunction]
#[pyo3(signature = (*, model_locations, seed_locations, deferred_locations, selected_keys))]
pub fn apply_deferred_locations<'py>(
    model_locations: &Bound<'py, PyDict>,
    seed_locations: &Bound<'py, PyDict>,
    deferred_locations: &Bound<'py, PyDict>,
    selected_keys: &Bound<'py, PyAny>,
) -> PyResult<(Bound<'py, PyDict>, Bound<'py, PyDict>)> {
    let selected = selected_keys
        .try_iter()?
        .map(|key| native_field(&key?, "name")?.extract::<String>())
        .collect::<PyResult<std::collections::HashSet<_>>>()?;
    let models = model_locations.copy()?;
    let seeds = seed_locations.copy()?;
    for (name, target) in deferred_locations.iter() {
        if selected.contains(&name.extract::<String>()?) {
            continue;
        }
        if models.contains(&name)? {
            models.set_item(&name, &target)?;
        }
        if seeds.contains(&name)? {
            seeds.set_item(&name, &target)?;
        }
    }
    Ok((models, seeds))
}
