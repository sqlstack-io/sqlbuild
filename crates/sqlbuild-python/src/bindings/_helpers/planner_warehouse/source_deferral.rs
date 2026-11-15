//! Source read deferral for managed source loaders.

use sqlbuild_host::entries::models::UserCodeHost;

use crate::bindings::_helpers::authored_values::project_vars::render_project_var_text;
use crate::bindings::_helpers::compiled_views::audit_reads;
use crate::bindings::_helpers::compiled_views::function_reads;
use crate::bindings::_helpers::compiled_views::model_reads;
use crate::bindings::_helpers::compiled_views::project_reads;
use crate::bindings::_helpers::compiled_views::source_reads;
use crate::bindings::_helpers::contracts::spec_resolution::resolve_target_config;
use crate::bindings::_helpers::model_config::typed_config::{config_value, model_values};
use crate::bindings::_helpers::planner_phases::objects::object;
use crate::bindings::_helpers::planner_phases::objects::{
    optional_text, planner_error, replace, text,
};
use crate::bindings::_helpers::planner_warehouse::constants::{
    MODEL_HOOK_CONFIG_KEYS, SQL_HOOK_ENTRY,
};
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use pyo3::types::PyDict as KeywordDict;
use pyo3::types::{PyDict, PyDictMethods, PyList, PyTuple};
use sqlbuild_analysis::planner_warehouse::main::source_call_names::source_call_names;
use sqlbuild_analysis::planner_warehouse::main::source_deferral_field::source_deferral_field;
use sqlbuild_python_base::boundary::models::KeywordArguments;
use sqlbuild_python_base::native_host::main::host_entry::host_entry;
use sqlbuild_python_base::records::main::construct::record_class::record_class;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use std::collections::BTreeSet;

fn managed_names(project: &Bound<'_, PyAny>) -> PyResult<BTreeSet<String>> {
    let mut names: BTreeSet<_> = BTreeSet::new();
    for source in project_reads::sources(project)?.try_iter()? {
        let entry = source_reads::source_entry(&source?)?;
        if !native_field(&entry, "loader")?.is_none() {
            names.insert(native_field(&entry, "name")?.extract()?);
        }
    }
    Ok(names)
}

fn sql_source_reads(sql: &Bound<'_, PyAny>, managed: &BTreeSet<String>) -> PyResult<Vec<String>> {
    let mut names: Vec<String> = Vec::new();
    for name in source_call_names(&sql.extract::<String>()?) {
        if managed.contains(&name) {
            names.push(name);
        }
    }
    Ok(names)
}

fn hook_source_reads(
    model: &Bound<'_, PyAny>,
    managed: &BTreeSet<String>,
) -> PyResult<Vec<String>> {
    let mut names: Vec<String> = Vec::new();
    let py = model.py();
    let hook_entry = record_class(py, SQL_HOOK_ENTRY)?;
    let source_kind = host_entry(py, UserCodeHost::SqlResourceRefKind)?.getattr("SOURCE")?;
    let values = model_values(model)?;
    for key in MODEL_HOOK_CONFIG_KEYS {
        let value = config_value(py, &values, key)?;
        let entries: Vec<Bound<'_, PyAny>> =
            if value.is_instance_of::<PyList>() || value.is_instance_of::<PyTuple>() {
                value.try_iter()?.collect::<PyResult<_>>()?
            } else {
                vec![value]
            };
        for entry in entries {
            if !entry.is_instance(&hook_entry)? {
                continue;
            }
            for reference in native_field(&entry, "reads")?.try_iter()? {
                let reference = reference?;
                if native_field(&reference, "kind")?.is(&source_kind) {
                    let name: String = native_field(&reference, "name")?.extract()?;
                    if managed.contains(&name) {
                        names.push(name);
                    }
                }
            }
        }
    }
    Ok(names)
}

fn selected_managed_refs(
    project: &Bound<'_, PyAny>,
    selected: &Bound<'_, PyAny>,
    managed: &BTreeSet<String>,
) -> PyResult<BTreeSet<String>> {
    let mut names: BTreeSet<_> = BTreeSet::new();
    for model in project_reads::models(project)?.try_iter()? {
        let model = model?;
        if !selected.contains(model_reads::model_key(&model)?)? {
            continue;
        }
        names.extend(sql_source_reads(&model_reads::query_sql(&model)?, managed)?);
        names.extend(hook_source_reads(&model, managed)?);
    }
    for function in project_reads::functions(project)?.try_iter()? {
        let function = function?;
        if selected.contains(function_reads::key(&function)?)? {
            names.extend(sql_source_reads(
                &function_reads::body_sql(&function)?,
                managed,
            )?);
        }
    }
    for audit in project_reads::audits(project)?.try_iter()? {
        let audit = audit?;
        let deps = audit_reads::scope_deps(&audit)?;
        if deps.is_truthy()? {
            let mut any = false;
            for dep in deps.try_iter()? {
                if selected.contains(dep?)? {
                    any = true;
                    break;
                }
            }
            if !any {
                continue;
            }
        }
        for reference in audit_reads::references(&audit)?.try_iter()? {
            let reference = reference?;
            let name: String = native_field(&reference, "ref_name")?.extract()?;
            if native_field(&reference, "ref_kind")?.eq("source")? && managed.contains(&name) {
                names.insert(name);
            }
        }
        names.extend(sql_source_reads(&audit_reads::sql_body(&audit)?, managed)?);
    }
    Ok(names)
}

fn resolve_target<'py>(
    project_config: &Bound<'py, PyAny>,
    local_config: &Bound<'py, PyAny>,
    target_name: Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    resolve_target_config(
        project_config.py(),
        project_config,
        local_config,
        &target_name,
    )
}

fn source_target_name<'py>(
    project: &Bound<'py, PyAny>,
    (project_config, local_config): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
    defer_sources_to: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py = project.py();
    if !defer_sources_to.is_none() {
        return Ok(defer_sources_to.clone());
    }
    let active = native_field(project, "effective_target_name")?;
    if active.is_none() || project_config.is_none() || local_config.is_none() {
        return Ok(py.None().into_bound(py));
    }
    let deferred = native_field(
        &resolve_target(project_config, local_config, active.clone())?,
        "defer_sources_to",
    )?;
    Ok(if deferred.is_truthy()? {
        deferred
    } else {
        active
    })
}

fn raw_source_entry<'py>(source: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let declared = source_reads::source_entry(source)?;
    let name = native_field(&declared, "name")?;
    for entry in native_field(&source_reads::source_file(source)?, "source_entries")?.try_iter()? {
        let entry = entry?;
        if native_field(&entry, "name")?.eq(&name)? {
            return Ok(entry);
        }
    }
    Ok(declared)
}

fn target_field<'py>(
    target_value: &Bound<'py, PyAny>,
    logical_value: &Bound<'py, PyAny>,
    effective_vars: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py = target_value.py();
    let target: Option<String> = target_value.extract()?;
    let logical: Option<String> = logical_value.extract()?;
    let resolved = source_deferral_field(
        (target.as_deref(), logical.as_deref()),
        |name: &str| -> PyResult<Option<String>> {
            if !effective_vars.contains(name)? {
                return Ok(None);
            }
            Ok(Some(
                object(
                    py,
                    render_project_var_text(
                        &effective_vars.get_item(name)?,
                        &text(py, &format!("source deferral variable '${{{name}}}'"))
                            .extract::<String>()?,
                    )?,
                )?
                .extract()?,
            ))
        },
    )?;
    Ok(optional_text(py, resolved.as_deref()))
}

fn entry_for_environment<'py>(
    entry: &Bound<'py, PyAny>,
    target: &Bound<'py, PyAny>,
    effective_vars: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    if !native_field(entry, "expression")?.is_none() {
        return Ok(entry.clone());
    }
    let mut database = native_field(entry, "database")?;
    if database.is_none() {
        database = target_field(
            &native_field(target, "database")?,
            &database,
            effective_vars,
        )?;
    }
    let mut schema = native_field(entry, "schema")?;
    if schema.is_none() {
        let loader_schema = native_field(target, "loader_schema")?;
        let target_schema = if loader_schema.is_truthy()? {
            loader_schema
        } else {
            native_field(target, "schema")?
        };
        schema = target_field(&target_schema, &schema, effective_vars)?;
    }
    replace(entry, &[("database", database), ("schema", schema)])
}

/// Resolve source reads for selected SQL, or for `read_source_names` when given.
pub(crate) fn build_source_read_map<'py>(
    py: pyo3::Python<'py>,
    options: Option<Bound<'py, KeywordDict>>,
) -> PyResult<Bound<'py, PyDict>> {
    let args = KeywordArguments::read(
        py,
        "build_source_read_map",
        options,
        (
            &[
                "project",
                "source_map",
                "selected_keys",
                "project_config",
                "local_config",
                "defer_sources_to",
            ],
            &["read_source_names"],
        ),
    )?;
    let project = &args.value("project")?;
    let source_map = args.value("source_map")?.downcast_into::<PyDict>()?;
    let source_map = &source_map;
    let selected_keys = &args.value("selected_keys")?;
    let project_config = &args.value("project_config")?;
    let local_config = &args.value("local_config")?;
    let defer_sources_to = &args.value("defer_sources_to")?;
    let read_source_names = &args.value("read_source_names")?;

    let py = project.py();
    let managed = managed_names(project)?;
    let selected: BTreeSet<String> = match Some(read_source_names).filter(|names| !names.is_none())
    {
        Some(names) => names
            .try_iter()?
            .map(|name| name?.extract::<String>())
            .collect::<PyResult<BTreeSet<_>>>()?
            .intersection(&managed)
            .cloned()
            .collect(),
        None => selected_managed_refs(project, selected_keys, &managed)?,
    };
    if selected.is_empty() {
        return Ok(source_map.clone());
    }
    let target_name =
        source_target_name(project, (project_config, local_config), defer_sources_to)?;
    if target_name.is_none() || project_config.is_none() || local_config.is_none() {
        return Ok(source_map.clone());
    }
    if !native_field(project_config, "targets")?.contains(&target_name)?
        && !native_field(local_config, "targets")?.contains(&target_name)?
    {
        return Err(planner_error(
            py,
            &format!("Unknown source deferral target '{}'", target_name.str()?),
            (None, None),
        ));
    }
    let target = resolve_target(project_config, local_config, target_name)?;
    let effective_vars = native_field(project, "effective_vars")?;
    let result = source_map.copy()?;
    for source in project_reads::sources(project)?.try_iter()? {
        let source = source?;
        let name: String =
            native_field(&source_reads::source_entry(&source)?, "name")?.extract()?;
        if !selected.contains(&name) {
            continue;
        }
        let entry = raw_source_entry(&source)?;
        result.set_item(
            name,
            entry_for_environment(&entry, &target, &effective_vars)?,
        )?;
    }
    Ok(result)
}

/// Add declared project sources that no selected SQL resource reads to a source map.
pub(crate) fn with_declared_source_reads<'py>(
    project: &Bound<'py, PyAny>,
    source_map: &Bound<'py, PyDict>,
    source_names: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyDict>> {
    let mut missing: BTreeSet<_> = BTreeSet::new();
    for name in source_names.try_iter()? {
        let name: String = name?.extract()?;
        if !source_map.contains(&name)? {
            missing.insert(name);
        }
    }
    if missing.is_empty() {
        return Ok(source_map.clone());
    }
    let result = source_map.copy()?;
    for source in project_reads::sources(project)?.try_iter()? {
        let entry = source_reads::source_entry(&source?)?;
        let name: String = native_field(&entry, "name")?.extract()?;
        if missing.contains(&name) {
            result.set_item(name, entry)?;
        }
    }
    Ok(result)
}
