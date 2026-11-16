//! Model ordering, selection and metadata at the migration discovery boundary.

use sqlbuild_host::entries::models::UserCodeHost;

use crate::planner_identity::main::build_function_local_hashes::build_function_local_hashes;
use crate::planner_identity::main::build_model_version_identity_metadata_json::build_model_version_identity_metadata_json;
use crate::planner_phases::main::inputs::runtime_field::runtime_field;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use pyo3::types::{PyDict, PyDictMethods};
use sqlbuild_analysis::graph::main::reachable_keys::reachable_keys;
use sqlbuild_analysis::planner_migrations::main::discovery_order::discovery_order;
use sqlbuild_python_base::native_host::main::host_entry::host_entry;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::main::fields::mapping_items::mapping_items;
use sqlbuild_python_project::compiled_views::main::compiled_models::deps::deps;
use sqlbuild_python_project::compiled_views::main::compiled_models::model_key::model_key;
use sqlbuild_python_project::compiled_views::main::compiled_models::name_object::name_object;
use sqlbuild_python_project::compiled_views::main::project::compiled_models::compiled_models;
use sqlbuild_python_project::graph::main::nodes::native_key::native_key;
use sqlbuild_python_project::planner_phases::main::object::object;
use sqlbuild_python_project::planner_scope::main::scope_data::scope_data;
use std::collections::BTreeMap;

pub(crate) fn ordered<'py>(runtime: &Bound<'py, PyAny>) -> PyResult<Vec<Bound<'py, PyAny>>> {
    let mut models: BTreeMap<String, Bound<'py, PyAny>> = BTreeMap::new();
    let mut dependencies: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for item in compiled_models(&runtime_field(runtime, |runtime| &runtime.project)?)?.try_iter()? {
        let item = item?;
        let name: String = name_object(&item)?.extract()?;
        let mut upstream: Vec<String> = Vec::new();
        for dependency in deps(&item)?.try_iter()? {
            let dependency = dependency?;
            if native_field(&dependency, "resource_type")?.eq("model")? {
                upstream.push(native_field(&dependency, "name")?.extract()?);
            }
        }
        dependencies.insert(name.clone(), upstream);
        models.insert(name, item);
    }
    Ok(discovery_order(&dependencies)
        .iter()
        .map(|name| models[name].clone())
        .collect())
}

pub(crate) fn triggered(
    models: &[Bound<'_, PyAny>],
    scope: &Bound<'_, PyAny>,
    snapshot: &Bound<'_, PyAny>,
    declared: &Bound<'_, PyDict>,
) -> PyResult<bool> {
    let scope = scope_data(scope)?;
    let selected: Vec<_> = scope.selected.iter().cloned().collect();
    let relevant = reachable_keys(&selected, &scope.upstream);
    let fingerprints = native_field(&native_field(snapshot, "fingerprints")?, "models")?;
    for item in models {
        let name = name_object(item)?;
        if relevant.contains(&native_key(&model_key(item)?)?)
            && !fingerprints.contains(&name)?
            && !declared.contains(&name)?
        {
            return Ok(true);
        }
    }
    Ok(false)
}

pub(crate) fn metadata<'py>(
    runtime: &Bound<'py, PyAny>,
    models: &[Bound<'py, PyAny>],
) -> PyResult<Bound<'py, PyDict>> {
    let py = runtime.py();
    let project = runtime_field(runtime, |runtime| &runtime.project)?;
    let functions = object(
        py,
        build_function_local_hashes(
            &sqlbuild_python_project::compiled_views::main::project::functions::functions(
                &project,
            )?,
            native_field(&project, "sql_analysis_dialect")?
                .extract::<Option<String>>()?
                .as_deref(),
        )?,
    )?;
    let identities = host_entry(py, UserCodeHost::BuildHookIdentities)?
        .call1((native_field(&project, "hook_functions")?,))?;
    let hooks = PyDict::new(py);
    for pair in mapping_items(&identities)?.try_iter()? {
        let pair = pair?;
        hooks.set_item(
            pair.get_item(0)?,
            native_field(&pair.get_item(1)?, "version_hash")?,
        )?;
    }
    let result = PyDict::new(py);
    for item in models {
        result.set_item(
            name_object(item)?,
            object(
                py,
                build_model_version_identity_metadata_json(
                    item,
                    functions.extract::<Option<Bound<PyAny>>>()?,
                    hooks.clone().into_any().extract::<Option<Bound<PyAny>>>()?,
                )?,
            )?,
        )?;
    }
    Ok(result)
}
