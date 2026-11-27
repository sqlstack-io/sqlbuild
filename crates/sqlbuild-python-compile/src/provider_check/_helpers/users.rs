//! The nodes that run a provider: its Python nodes, its loaders' sources, models using its hooks or materializations.

use std::collections::BTreeSet;

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods, PyString};
use sqlbuild_python_base::orchestration_reads::main::missing_secrets::missing_secrets;
use sqlbuild_python_base::records::main::fields::field::field;
use sqlbuild_python_front::model_config::main::config::config_value::config_value;
use sqlbuild_python_front::model_config::main::config::model_values::model_values;
use sqlbuild_python_project::compiled_views::main::compiled_models::model_name::model_name;
use sqlbuild_python_project::compiled_views::main::project::compiled_models::compiled_models;
use sqlbuild_python_project::graph::models::ProjectSelection;

use crate::provider_check::constants::{HOOK_SETTINGS, OWN_NODES};

/// Providers whose required secrets are unset, by name, with their file and variables.
pub(crate) fn missing_providers(
    discovered: &Bound<'_, PyAny>,
) -> PyResult<Vec<(String, String, Vec<String>)>> {
    let mut missing: Vec<(String, String, Vec<String>)> = Vec::new();
    for provider in field(discovered, "providers")?.try_iter()? {
        let provider = provider?;
        let variables: Vec<String> = missing_secrets(&field(&provider, "provider_class")?)?;
        if !variables.is_empty() {
            missing.push((
                field(&provider, "name")?.extract()?,
                field(&provider, "relative_path")?.str()?.extract()?,
                variables,
            ));
        }
    }
    Ok(missing)
}

/// The names of `collection`'s items that inject `provider`.
fn consumers_of(
    discovered: &Bound<'_, PyAny>,
    collection: &str,
    provider: &str,
) -> PyResult<Vec<String>> {
    let mut names: Vec<String> = Vec::new();
    let Some(items) = discovered.getattr_opt(collection)? else {
        return Ok(names);
    };
    for item in items.try_iter()? {
        let item = item?;
        for usage in field(&item, "provider_usages")?.try_iter()? {
            if field(&usage?, "provider_name")?.extract::<String>()? == provider {
                names.push(field(&item, "name")?.extract()?);
                break;
            }
        }
    }
    Ok(names)
}

/// The hook a model's hook entry names: a name, or a mapping with a `name`; `None` for inline SQL.
fn hook_name(entry: &Bound<'_, PyAny>) -> PyResult<Option<String>> {
    if entry.is_instance_of::<PyString>() {
        return Ok(Some(entry.extract()?));
    }
    if !entry.is_instance_of::<PyDict>() {
        return Ok(None);
    }
    match entry.cast::<PyDict>()?.get_item("name")? {
        Some(name) if name.is_instance_of::<PyString>() => Ok(Some(name.extract()?)),
        _ => Ok(None),
    }
}

/// The hooks a model's `pre_hooks` or `post_hooks` value names: one entry, or a list of them.
fn hook_names(hooks: &Bound<'_, PyAny>) -> PyResult<Vec<String>> {
    if hooks.is_none() {
        return Ok(Vec::new());
    }
    if hooks.is_instance_of::<PyString>() || hooks.is_instance_of::<PyDict>() {
        return Ok(hook_name(hooks)?.into_iter().collect());
    }
    let mut names: Vec<String> = Vec::new();
    for entry in hooks.try_iter()? {
        names.extend(hook_name(&entry?)?);
    }
    Ok(names)
}

/// `model:<name>` of each compiled model whose hooks call one of `hooks` or that a custom `materializations` builds.
fn model_users(
    project: &Bound<'_, PyAny>,
    (hooks, materializations): (&[String], &[String]),
) -> PyResult<Vec<String>> {
    let py: Python<'_> = project.py();
    let mut keys: Vec<String> = Vec::new();
    if hooks.is_empty() && materializations.is_empty() {
        return Ok(keys);
    }
    for model in compiled_models(project)?.try_iter()? {
        let model = model?;
        let values = model_values(&model)?;
        let mut called: Vec<String> = Vec::new();
        for setting in HOOK_SETTINGS {
            called.extend(hook_names(&config_value(py, &values, setting)?)?);
        }
        let materialized = config_value(py, &values, "materialized")?;
        let custom: bool = materialized.is_instance_of::<PyString>()
            && materializations.contains(&materialized.extract::<String>()?);
        if custom || called.iter().any(|name| hooks.contains(name)) {
            keys.push(format!("model:{}", model_name(&model)?));
        }
    }
    Ok(keys)
}

/// Every node key that runs `provider`: its Python nodes, the sources its loaders fill, and models calling its hooks or materializations.
pub(crate) fn provider_users(
    (discovered, project): (&Bound<'_, PyAny>, &Bound<'_, PyAny>),
    selection: &ProjectSelection,
    provider: &str,
) -> PyResult<BTreeSet<String>> {
    let mut users: BTreeSet<String> = BTreeSet::new();
    for (collection, kind) in OWN_NODES {
        for name in consumers_of(discovered, collection, provider)? {
            let key: String = selection
                .node_key(&name)
                .unwrap_or(format!("{kind}:{name}"));
            users.insert(key);
        }
    }
    for name in consumers_of(discovered, "loader_functions", provider)? {
        users.extend(selection.run_key(&name));
    }
    let hooks = consumers_of(discovered, "hook_functions", provider)?;
    let materializations = consumers_of(discovered, "materialization_files", provider)?;
    users.extend(model_users(project, (&hooks, &materializations))?);
    Ok(users)
}
