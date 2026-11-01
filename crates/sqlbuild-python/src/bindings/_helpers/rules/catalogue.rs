//! The validated rule catalogue, the active rules it selects and resolved ignore selectors.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyModule, PyModuleMethods, PyResult, Python};
use pyo3::types::{
    PyBool, PyCFunction, PyDict, PyDictMethods, PyInt, PyString, PyTuple, PyTupleMethods,
};
use pyo3::{pyfunction, wrap_pyfunction};
use sqlbuild_host::entries::models::UserCodeHost;
use sqlbuild_rules::models::RulesConfig;
use sqlbuild_rules::rule_catalogue::main::sorted_catalogue_order::sorted_catalogue_order;
use sqlbuild_rules::rule_catalogue::models::{CatalogueRule, OptionKind, OptionSpec, OptionValue};

use crate::bindings::_helpers::boundary::panics::compiler_guard;
use crate::bindings::_helpers::boundary::python_calls::{call_with, imported, keywords};
use crate::bindings::_helpers::native_host::host;
use crate::bindings::_helpers::planner_scope::scope::build_planner_scope;
use crate::bindings::_helpers::records::fields::{field as record_field, path_posix, replaced};
use crate::bindings::_helpers::rules::builtin_catalogue::builtin_rules;
use crate::bindings::_helpers::rules::config_json::asdict_json;
use crate::bindings::_helpers::rules::custom_rule_evidence::verify_custom_rules as verify_rule_sources;
use crate::bindings::_helpers::rules::fact_replay::{project_attribute, project_models};
use crate::bindings::_helpers::rules::selection::{rules_error, selected_rule_codes};

const DATACLASSES_MODULE: &str = "dataclasses";
const JSON_MODULE: &str = "json";
const BUILTINS_MODULE: &str = "builtins";
const RULES_DIRECTORY: &str = "rules";
const CUSTOM_RULE_PREFIX: &str = "XSQBR";
const BUILTIN_CHECK_MESSAGE: &str = "built-in rules execute only through the native engine";

/// The Python rules configuration as the native configuration model.
pub(crate) fn native_rules_config(config: &Bound<'_, PyAny>) -> PyResult<RulesConfig> {
    let py: Python<'_> = config.py();
    if let Some(fields) = asdict_json(config)? {
        return serde_json::from_value(fields).map_err(|error| {
            rules_error(py, &format!("invalid rules configuration: {error}"), None)
        });
    }
    let fields: Bound<'_, PyAny> = imported(py, DATACLASSES_MODULE, "asdict")?.call1((config,))?;
    let encoded: String = imported(py, JSON_MODULE, "dumps")?
        .call(
            (fields,),
            Some(&keywords(
                py,
                &[("default", imported(py, BUILTINS_MODULE, "str")?)],
            )?),
        )?
        .extract()?;
    serde_json::from_str(&encoded)
        .map_err(|error| rules_error(py, &format!("invalid rules configuration: {error}"), None))
}

/// Whether any selector, option, exception or ignore of `config` names a custom rule.
pub(crate) fn references_custom_rules(config: &Bound<'_, PyAny>) -> PyResult<bool> {
    let mut selectors: Vec<String> = Vec::new();
    let select: Option<Vec<String>> = record_field(config, "select")?.extract()?;
    selectors.extend(select.unwrap_or_default());
    for field in ["ignore", "rule_options"] {
        for selector in record_field(config, field)?.try_iter()? {
            selectors.push(selector?.extract()?);
        }
    }
    for entry in record_field(config, "rule_exceptions")?.try_iter()? {
        selectors.push(record_field(&entry?, "rule")?.extract()?);
    }
    for entry in record_field(config, "rule_ignores")?.try_iter()? {
        for rule in record_field(&entry?, "rules")?.try_iter()? {
            selectors.push(rule?.extract()?);
        }
    }
    Ok(selectors
        .iter()
        .any(|selector| selector.starts_with(CUSTOM_RULE_PREFIX)))
}

fn option_kind(value_type: &Bound<'_, PyAny>) -> PyResult<OptionKind> {
    let py: Python<'_> = value_type.py();
    let kinds: [(Bound<'_, PyAny>, OptionKind); 4] = [
        (py.get_type::<PyBool>().into_any(), OptionKind::Bool),
        (py.get_type::<PyInt>().into_any(), OptionKind::Int),
        (py.get_type::<PyString>().into_any(), OptionKind::Str),
        (py.get_type::<PyTuple>().into_any(), OptionKind::Tuple),
    ];
    for (class, kind) in kinds {
        if value_type.is(&class) {
            return Ok(kind);
        }
    }
    let name: String = match value_type.getattr("__name__") {
        Ok(name) => name.extract()?,
        Err(_) => value_type.repr()?.extract()?,
    };
    Ok(OptionKind::Other(name))
}

fn option_value(value: &Bound<'_, PyAny>) -> PyResult<OptionValue> {
    if value.is_instance_of::<PyBool>() {
        return Ok(OptionValue::Bool(value.extract()?));
    }
    if value.is_instance_of::<PyInt>() {
        return Ok(value
            .extract::<i64>()
            .map_or_else(|_| OptionValue::Other("int".into()), OptionValue::Int));
    }
    if value.is_instance_of::<PyString>() {
        return Ok(OptionValue::Str(value.extract()?));
    }
    if let Ok(items) = value.cast::<PyTuple>() {
        return Ok(OptionValue::Tuple(
            items
                .iter()
                .map(|item| option_value(&item))
                .collect::<PyResult<_>>()?,
        ));
    }
    Ok(OptionValue::Other(
        value.get_type().getattr("__name__")?.extract()?,
    ))
}

fn optional_integer(option: &Bound<'_, PyAny>, field: &str) -> PyResult<Option<i64>> {
    record_field(option, field)?.extract()
}

fn option_spec(option: &Bound<'_, PyAny>) -> PyResult<OptionSpec> {
    let item_type: Bound<'_, PyAny> = record_field(option, "item_type")?;
    Ok(OptionSpec {
        name: record_field(option, "name")?.extract()?,
        description: record_field(option, "description")?.extract()?,
        value_type: option_kind(&record_field(option, "value_type")?)?,
        default: option_value(&record_field(option, "default")?)?,
        choices: record_field(option, "choices")?
            .try_iter()?
            .map(|choice| option_value(&choice?))
            .collect::<PyResult<_>>()?,
        minimum: optional_integer(option, "minimum")?,
        maximum: optional_integer(option, "maximum")?,
        minimum_items: optional_integer(option, "minimum_items")?,
        item_type: if item_type.is_none() {
            None
        } else {
            Some(option_kind(&item_type)?)
        },
    })
}

fn catalogue_rule(rule: &Bound<'_, PyAny>) -> PyResult<CatalogueRule> {
    Ok(CatalogueRule {
        code: record_field(rule, "code")?.extract()?,
        options: record_field(rule, "options")?
            .try_iter()?
            .map(|option| option_spec(&option?))
            .collect::<PyResult<_>>()?,
    })
}

fn configured_options(
    config: &Bound<'_, PyAny>,
) -> PyResult<BTreeMap<String, BTreeMap<String, OptionValue>>> {
    let mut configured: BTreeMap<String, BTreeMap<String, OptionValue>> = BTreeMap::new();
    let options: Bound<'_, PyDict> = record_field(config, "rule_options")?.cast_into::<PyDict>()?;
    for (code, values) in options.iter() {
        let values: Bound<'_, PyDict> = values.cast_into::<PyDict>()?;
        let mut entries: BTreeMap<String, OptionValue> = BTreeMap::new();
        for (name, value) in values.iter() {
            entries.insert(name.extract()?, option_value(&value)?);
        }
        configured.insert(code.extract()?, entries);
    }
    Ok(configured)
}

fn custom_rules<'py>(project_dir: &Bound<'py, PyAny>) -> PyResult<Vec<Bound<'py, PyAny>>> {
    let py: Python<'py> = project_dir.py();
    let root: PathBuf = project_dir.extract()?;
    let directory: PathBuf = root.join(RULES_DIRECTORY);
    if !directory.is_dir() {
        return Ok(Vec::new());
    }
    let escapes = |directory: &Path| -> bool {
        match (directory.canonicalize(), root.canonicalize()) {
            (Ok(resolved), Ok(root)) => !resolved.starts_with(root),
            _ => true,
        }
    };
    if escapes(&directory) {
        return Err(rules_error(
            py,
            &format!("rule path escapes the project: {RULES_DIRECTORY}"),
            None,
        ));
    }
    call_with(
        &host::entry(py, UserCodeHost::LoadCustomRules)?,
        &[("project_dir", project_dir.clone())],
    )?
    .try_iter()?
    .collect()
}

/// The validated built-in and, when requested, custom catalogue sorted by rule code.
pub(crate) fn rule_catalogue<'py>(
    config: &Bound<'py, PyAny>,
    project_dir: &Bound<'py, PyAny>,
    include_custom: bool,
) -> PyResult<Vec<Bound<'py, PyAny>>> {
    let py: Python<'py> = config.py();
    let check: Bound<'py, PyCFunction> = PyCFunction::new_closure(
        py,
        None,
        None,
        |arguments: &Bound<'_, PyTuple>, _| -> PyResult<()> {
            Err(rules_error(arguments.py(), BUILTIN_CHECK_MESSAGE, None))
        },
    )?;
    let mut rules: Vec<Bound<'py, PyAny>> = builtin_rules(py, check.into_any())?;
    if include_custom {
        rules.extend(custom_rules(project_dir)?);
    }
    let views: Vec<CatalogueRule> = rules.iter().map(catalogue_rule).collect::<PyResult<_>>()?;
    let configured = configured_options(config)?;
    let order: Vec<usize> = compiler_guard(|| Ok(sorted_catalogue_order(&views, &configured)))?
        .map_err(|message| rules_error(py, &message, None))?;
    Ok(order
        .into_iter()
        .map(|index| rules[index].clone())
        .collect())
}

/// The catalogue rules the configuration activates, in native selection order.
pub(crate) fn select_rules<'py>(
    catalogue: &[Bound<'py, PyAny>],
    config: &Bound<'py, PyAny>,
    project_dir: &Bound<'py, PyAny>,
    custom_payloads: Option<Bound<'py, PyAny>>,
) -> PyResult<Vec<Bound<'py, PyAny>>> {
    let py: Python<'py> = config.py();
    let payloads: Bound<'py, PyAny> = match custom_payloads {
        Some(payloads) if !payloads.is_none() => payloads,
        _ => call_with(
            &host::entry(py, UserCodeHost::CustomRulePayloads)?,
            &[
                ("catalogue", PyTuple::new(py, catalogue)?.into_any()),
                ("project_dir", project_dir.clone()),
            ],
        )?,
    };
    let mut by_code: BTreeMap<String, Bound<'py, PyAny>> = BTreeMap::new();
    for rule in catalogue {
        by_code.insert(record_field(rule, "code")?.extract()?, rule.clone());
    }
    selected_rule_codes(config, &payloads)?
        .into_iter()
        .map(|code| {
            by_code.get(&code).cloned().ok_or_else(|| {
                rules_error(
                    py,
                    &format!("native compiler rules returned unknown rule {code}"),
                    None,
                )
            })
        })
        .collect()
}

/// Reject non-hermetic imports in the source closure of every selected custom rule.
pub(crate) fn verify_custom_rules<'py>(
    rules: &[Bound<'py, PyAny>],
    project_dir: &Bound<'py, PyAny>,
) -> PyResult<()> {
    let py: Python<'py> = project_dir.py();
    verify_rule_sources(PyTuple::new(py, rules)?.as_any(), project_dir)?;
    Ok(())
}

/// The validated built-in and custom rule catalogue sorted by code.
#[pyfunction]
#[pyo3(signature = (*, config, project_dir, include_custom = true))]
pub(crate) fn build_rule_catalogue<'py>(
    config: &Bound<'py, PyAny>,
    project_dir: &Bound<'py, PyAny>,
    include_custom: bool,
) -> PyResult<Bound<'py, PyTuple>> {
    PyTuple::new(
        config.py(),
        rule_catalogue(config, project_dir, include_custom)?,
    )
}

/// The catalogue rules the configuration activates; `custom_payloads` describes custom rules.
#[pyfunction]
#[pyo3(signature = (*, catalogue, config, project_dir, custom_payloads = None))]
fn select_catalogue_rules<'py>(
    catalogue: &Bound<'py, PyAny>,
    config: &Bound<'py, PyAny>,
    project_dir: &Bound<'py, PyAny>,
    custom_payloads: Option<Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyTuple>> {
    let rules: Vec<Bound<'py, PyAny>> = catalogue.try_iter()?.collect::<PyResult<_>>()?;
    PyTuple::new(
        config.py(),
        select_rules(&rules, config, project_dir, custom_payloads)?,
    )
}

/// Each model, function, seed and source key's relative path as POSIX text.
fn resource_paths(project: &Bound<'_, PyAny>) -> PyResult<HashMap<(String, String), String>> {
    let mut paths: HashMap<(String, String), String> = HashMap::new();
    for model in project_models(project)? {
        paths.insert(model.key, model.posix);
    }
    for (collection, file) in [
        ("functions", None),
        ("seeds", Some("seed_file")),
        ("sources", Some("source_file")),
    ] {
        for resource in project_attribute(project, collection)?.try_iter()? {
            let resource: Bound<'_, PyAny> = resource?;
            let owner: Bound<'_, PyAny> = match file {
                Some(file) => record_field(&resource, file)?,
                None => resource.clone(),
            };
            paths.insert(
                object_key(&record_field(&resource, "key")?)?,
                path_posix(&record_field(&owner, "relative_path")?)?,
            );
        }
    }
    Ok(paths)
}

/// `(resource type, name)` of a `CompiledObjectKey`.
fn object_key(key: &Bound<'_, PyAny>) -> PyResult<(String, String)> {
    Ok((
        record_field(key, "resource_type")?.str()?.extract()?,
        record_field(key, "name")?.extract()?,
    ))
}

/// `config` with every selector-scoped rule ignore resolved into deterministic finding paths.
pub(crate) fn resolved_ignore_selectors<'py>(
    config: &Bound<'py, PyAny>,
    project: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = config.py();
    let ignores: Vec<Bound<'py, PyAny>> = record_field(config, "rule_ignores")?
        .try_iter()?
        .collect::<PyResult<_>>()?;
    let mut scoped: bool = false;
    for ignore in &ignores {
        scoped |= record_field(ignore, "selectors")?.is_truthy()?;
    }
    if !scoped {
        return Ok(config.clone());
    }
    let path_by_key: HashMap<(String, String), String> = resource_paths(project)?;
    let mut resolved: Vec<Bound<'py, PyAny>> = Vec::with_capacity(ignores.len());
    for ignore in ignores {
        let selectors: Bound<'py, PyAny> = record_field(&ignore, "selectors")?;
        if !selectors.is_truthy()? {
            resolved.push(ignore);
            continue;
        }
        let selected: Bound<'py, PyAny> = build_planner_scope(
            py,
            Some(keywords(
                py,
                &[("project", project.clone()), ("select", selectors.clone())],
            )?),
        )?;
        let mut selector_paths: BTreeSet<String> = BTreeSet::new();
        for key in record_field(&selected, "selected_keys")?.try_iter()? {
            if let Some(path) = path_by_key.get(&object_key(&key?)?) {
                selector_paths.insert(path.clone());
            }
        }
        if selector_paths.is_empty() {
            let joined: Vec<String> = selectors.extract()?;
            return Err(rules_error(
                py,
                &format!(
                    "rule ignore selectors resolve no resources with paths: {}",
                    joined.join(", ")
                ),
                None,
            ));
        }
        for path in record_field(&ignore, "paths")?.try_iter()? {
            selector_paths.insert(path?.extract()?);
        }
        resolved.push(replaced(
            &ignore,
            &[
                ("paths", PyTuple::new(py, selector_paths)?.into_any()),
                ("selectors", PyTuple::empty(py).into_any()),
            ],
        )?);
    }
    replaced(
        config,
        &[("rule_ignores", PyTuple::new(py, resolved)?.into_any())],
    )
}

/// `config` with selector-scoped rule ignores resolved against the compiled project.
#[pyfunction]
#[pyo3(signature = (*, config, project))]
fn resolve_rule_ignore_selectors<'py>(
    config: &Bound<'py, PyAny>,
    project: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    resolved_ignore_selectors(config, project)
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(build_rule_catalogue, module)?)?;
    module.add_function(wrap_pyfunction!(select_catalogue_rules, module)?)?;
    module.add_function(wrap_pyfunction!(resolve_rule_ignore_selectors, module)?)?;
    Ok(())
}
