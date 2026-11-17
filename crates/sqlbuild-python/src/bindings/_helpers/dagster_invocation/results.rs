//! Dagster event plans from execution documents, integration results and DAG artifacts.

use std::collections::HashSet;

use pyo3::exceptions::PyValueError;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyBool, PyDict, PyDictMethods, PyList, PyString, PyTuple};
use pyo3::{IntoPyObject, pyfunction};
use sqlbuild_core::dagster_invocation::main::normalize_check_name::normalize_check_name;
use sqlbuild_core::dagster_invocation::main::topological_order::topological_order;
use sqlbuild_host::entries::models::ThirdPartyHost;

use crate::bindings::_helpers::dagster_invocation::access::{
    AssetPath, CheckKey, asset_enabled, asset_path, check_enabled, check_key_set, constant,
    constant_contains, get, items, merged, node_with_id, nodes_by_id, path_set, string_list,
    text_of, value_or,
};
use sqlbuild_python_base::boundary::main::imported::imported;
use sqlbuild_python_base::boundary::models::KeywordArguments;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::main::fields::mapping_get::mapping_get;
use sqlbuild_python_base::records::main::fields::mapping_items::mapping_items;

const CHECK_KIND_FALLBACK: &str = "check";
const CHECK_NAME_JOIN: &str = "__";
const EXECUTION_DOCUMENT_VERSION: i64 = 1;
const CHECK_ASSET_KINDS: [&str; 5] = ["model", "source", "seed", "udf", "table_fn"];
const MATERIALIZE: &str = "materialize";
const MATERIALIZATION: &str = "materialization";
const CHECK: &str = "check";
const AUDIT_CHECK_KIND: &str = "audit";
/// The status of an audit that did not run; Dagster gets no check result for it.
pub(crate) const SKIPPED_CHECK_STATUS: &str = "skipped";

/// DAG nodes upstream first, or in artifact order when edges form a cycle.
pub(crate) fn sorted_nodes<'py>(dag: &Bound<'py, PyAny>) -> PyResult<Vec<Bound<'py, PyAny>>> {
    let nodes: Vec<Bound<'py, PyAny>> = items(dag, "nodes")?;
    let ids: Vec<String> = nodes
        .iter()
        .map(|node| text_of(node, "id"))
        .collect::<PyResult<_>>()?;
    let mut edges: Vec<(String, String)> = Vec::new();
    for edge in items(dag, "edges")? {
        edges.push((text_of(&edge, "from_id")?, text_of(&edge, "to_id")?));
    }
    Ok(match topological_order(&ids, &edges) {
        Some(order) => order
            .into_iter()
            .map(|position| nodes[position].clone())
            .collect(),
        None => nodes,
    })
}

/// The translated Dagster check name, or one built from the check's kind, name and target.
pub(crate) fn dagster_check_name(check: &Bound<'_, PyAny>) -> PyResult<String> {
    let py: Python<'_> = check.py();
    let field: Bound<'_, PyAny> = constant(py, ThirdPartyHost::DagsterCheckNameField)?;
    let translated: Bound<'_, PyAny> = mapping_get(check, field)?;
    if !translated.is_none() {
        return translated.str()?.extract();
    }
    let mut parts: Vec<String> = vec![
        value_or(
            check,
            "kind",
            PyString::new(py, CHECK_KIND_FALLBACK).into_any(),
        )?
        .str()?
        .extract()?,
        value_or(
            check,
            "name",
            PyString::new(py, CHECK_KIND_FALLBACK).into_any(),
        )?
        .str()?
        .extract()?,
    ];
    for attached in ["attached_column_name", "attached_target_name"] {
        let value: Bound<'_, PyAny> = get(check, attached)?;
        if !value.is_none() {
            parts.push(check.get_item(attached)?.str()?.extract()?);
            break;
        }
    }
    Ok(parts
        .iter()
        .filter(|part| !part.is_empty())
        .map(|part| normalize_check_name(part))
        .collect::<Vec<String>>()
        .join(CHECK_NAME_JOIN))
}

/// A metadata value with enums replaced by their values and containers made plain.
fn metadata_value<'py>(value: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = value.py();
    if value.is_instance(&imported(py, "enum", "Enum")?)? {
        return native_field(value, "value");
    }
    if value.is_instance(&imported(py, "collections.abc", "Mapping")?)? {
        let plain: Bound<'py, PyDict> = PyDict::new(py);
        for pair in mapping_items(value)?.try_iter()? {
            let (key, item): (Bound<'py, PyAny>, Bound<'py, PyAny>) = pair?.extract()?;
            plain.set_item(key.str()?, metadata_value(&item)?)?;
        }
        return Ok(plain.into_any());
    }
    if value.is_instance_of::<PyList>() || value.is_instance_of::<PyTuple>() {
        let plain: Vec<Bound<'py, PyAny>> = value
            .try_iter()?
            .map(|item| metadata_value(&item?))
            .collect::<PyResult<_>>()?;
        return Ok(PyList::new(py, plain)?.into_any());
    }
    Ok(value.clone())
}

/// A record's non-null metadata, without check internals.
fn metadata_from_mapping<'py>(value: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyDict>> {
    let py: Python<'py> = value.py();
    let excluded: Bound<'py, PyAny> = constant(py, ThirdPartyHost::CheckMetadataExcludedKeys)?;
    let metadata: Bound<'py, PyDict> = PyDict::new(py);
    for pair in mapping_items(value)?.try_iter()? {
        let (key, item): (Bound<'py, PyAny>, Bound<'py, PyAny>) = pair?.extract()?;
        if item.is_none() || excluded.contains(&key)? {
            continue;
        }
        metadata.set_item(key.str()?, metadata_value(&item)?)?;
    }
    Ok(metadata)
}

/// One Dagster event: `(kind, asset_path, metadata, check_name, passed, warn)`.
fn plan<'py>(
    py: Python<'py>,
    (kind, path): (&str, &AssetPath),
    metadata: Bound<'py, PyDict>,
    check: Option<(&str, bool, bool)>,
) -> PyResult<Bound<'py, PyAny>> {
    let none = || py.None().into_bound(py);
    let (name, passed, warn) = match check {
        Some((name, passed, warn)) => (
            PyString::new(py, name).into_any(),
            PyBool::new(py, passed).to_owned().into_any(),
            PyBool::new(py, warn).to_owned().into_any(),
        ),
        None => (none(), none(), none()),
    };
    Ok(PyTuple::new(
        py,
        [
            PyString::new(py, kind).into_any(),
            string_list(py, path)?,
            metadata.into_any(),
            name,
            passed,
            warn,
        ],
    )?
    .into_any())
}

fn command_text(command: &[String]) -> String {
    command.join(" ")
}

/// The loader feeding a source, with the loader's own execution facts.
fn loader_result<'py>(
    dag: &Bound<'py, PyAny>,
    (source_node, payload_asset): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
    loader_name: &str,
) -> PyResult<Option<(String, Bound<'py, PyDict>)>> {
    let py: Python<'py> = dag.py();
    let nodes = nodes_by_id(dag)?;
    let loader_kind: String = constant(py, ThirdPartyHost::LoaderNodeKind)?.extract()?;
    let source_id: String = text_of(source_node, "id")?;
    for edge in items(dag, "edges")? {
        if text_of(&edge, "to_id")? != source_id {
            continue;
        }
        let Some(upstream) = node_with_id(&nodes, &text_of(&edge, "from_id")?) else {
            continue;
        };
        if text_of(upstream, "kind")? != loader_kind || text_of(upstream, "name")? != loader_name {
            continue;
        }
        let facts: Bound<'py, PyDict> = PyDict::new(py);
        facts.set_item("kind", "loader")?;
        facts.set_item("name", get(upstream, "name")?)?;
        facts.set_item("resource_id", get(upstream, "id")?)?;
        facts.set_item("source", get(source_node, "name")?)?;
        facts.set_item("source_relation", get(payload_asset, "target")?)?;
        facts.set_item("status", get(payload_asset, "status")?)?;
        return Ok(Some((text_of(upstream, "id")?, facts)));
    }
    Ok(None)
}

/// Dagster selection facts of the running asset or check.
struct SelectionFacts {
    paths: HashSet<AssetPath>,
    check_keys: HashSet<CheckKey>,
    explicit_checks: bool,
}

impl SelectionFacts {
    fn from_arguments(arguments: &KeywordArguments<'_>) -> PyResult<Self> {
        Ok(Self {
            paths: path_set(&arguments.value("selected_asset_paths")?)?,
            check_keys: check_key_set(&arguments.value("selected_check_keys")?)?,
            explicit_checks: arguments.flag("check_selection_is_explicit")?,
        })
    }

    fn check_only(&self) -> bool {
        !self.check_keys.is_empty() && self.paths.is_empty()
    }
}

/// How check results are filtered.
struct CheckFilter<'a> {
    selection: &'a SelectionFacts,
    selected_paths: &'a HashSet<AssetPath>,
    emitted_asset_paths: Option<&'a HashSet<AssetPath>>,
}

/// Whether a check result is a warning rather than an error.
pub(crate) fn warn_severity(check: &Bound<'_, PyAny>) -> PyResult<bool> {
    let py: Python<'_> = check.py();
    let warning: String = constant(py, ThirdPartyHost::WarningCheckSeverity)?.extract()?;
    let insufficient: String = constant(py, ThirdPartyHost::InsufficientCheckStatus)?.extract()?;
    let status: String = text_of(check, "status")?;
    Ok(status == insufficient || status == warning || text_of(check, "severity")? == warning)
}

fn dag_check_for<'py>(
    dag: &Bound<'py, PyAny>,
    check: &Bound<'py, PyAny>,
) -> PyResult<Option<Bound<'py, PyAny>>> {
    let mut wanted: Bound<'py, PyAny> = get(check, "dag_check_id")?;
    if !wanted.is_truthy()? {
        wanted = get(check, "check_id")?;
    }
    for dag_check in items(dag, "checks")? {
        if get(&dag_check, "id")?.eq(&wanted)? {
            return Ok(Some(dag_check));
        }
    }
    singular_audit_check(dag, check)
}

/// The DAG check `audit:<name>` of a singular audit whose result names where it ran.
fn singular_audit_check<'py>(
    dag: &Bound<'py, PyAny>,
    check: &Bound<'py, PyAny>,
) -> PyResult<Option<Bound<'py, PyAny>>> {
    if text_of(check, "kind")? != AUDIT_CHECK_KIND {
        return Ok(None);
    }
    let wanted: String = format!("{AUDIT_CHECK_KIND}:{}", text_of(check, "name")?);
    for dag_check in items(dag, "checks")? {
        if text_of(&dag_check, "id")? == wanted && get(&dag_check, "attachment_kind")?.is_none() {
            return Ok(Some(dag_check));
        }
    }
    Ok(None)
}

fn asset_ids_for_check(dag: &Bound<'_, PyAny>, check: &Bound<'_, PyAny>) -> PyResult<Vec<String>> {
    let asset_name: Bound<'_, PyAny> = get(check, "asset_name")?;
    if asset_name.is_none() {
        return Ok(Vec::new());
    }
    let name: String = asset_name.str()?.extract()?;
    for kind in CHECK_ASSET_KINDS {
        if let Some(node) = node_by_kind_and_name(dag, (kind, &name))? {
            return Ok(vec![text_of(&node, "id")?]);
        }
    }
    Ok(Vec::new())
}

fn check_plans<'py>(
    (dag, nodes): (&Bound<'py, PyAny>, &[(String, Bound<'py, PyAny>)]),
    check: &Bound<'py, PyAny>,
    (filter, seen): (&CheckFilter<'_>, HashSet<CheckKey>),
) -> PyResult<(Vec<Bound<'py, PyAny>>, HashSet<CheckKey>)> {
    let mut seen: HashSet<CheckKey> = seen;
    let py: Python<'py> = dag.py();
    if get(check, "status")?.eq(SKIPPED_CHECK_STATUS)? {
        return Ok((Vec::new(), seen));
    }
    let dag_check: Option<Bound<'py, PyAny>> = dag_check_for(dag, check)?;
    let (asset_ids, check_name): (Vec<String>, String) = match &dag_check {
        Some(dag_check) if !check_enabled(dag_check)? => return Ok((Vec::new(), seen)),
        Some(dag_check) => (
            items(dag_check, "checked_asset_ids")?
                .iter()
                .map(|id| id.str()?.extract())
                .collect::<PyResult<_>>()?,
            dagster_check_name(dag_check)?,
        ),
        None => (asset_ids_for_check(dag, check)?, dagster_check_name(check)?),
    };
    let mut plans: Vec<Bound<'py, PyAny>> = Vec::new();
    for asset_id in asset_ids {
        let Some(node) = node_with_id(nodes, &asset_id) else {
            continue;
        };
        if !asset_enabled(node)? {
            continue;
        }
        let path: AssetPath = asset_path(node)?;
        let key: CheckKey = (path.clone(), check_name.clone());
        if filter
            .emitted_asset_paths
            .is_some_and(|emitted| !emitted.contains(&path))
        {
            continue;
        }
        if filter.selection.explicit_checks && !filter.selection.check_keys.contains(&key) {
            continue;
        }
        if !filter.selection.explicit_checks
            && !filter.selected_paths.is_empty()
            && !filter.selected_paths.contains(&path)
        {
            continue;
        }
        if !seen.insert(key) {
            continue;
        }
        plans.push(plan(
            py,
            (CHECK, &path),
            metadata_from_mapping(check)?,
            Some((
                &check_name,
                get(check, "passed")?.is_truthy()?,
                warn_severity(check)?,
            )),
        )?);
    }
    Ok((plans, seen))
}

fn node_by_kind_and_name<'py>(
    dag: &Bound<'py, PyAny>,
    (kind, name): (&str, &str),
) -> PyResult<Option<Bound<'py, PyAny>>> {
    let mut found: Option<Bound<'py, PyAny>> = None;
    for node in items(dag, "nodes")? {
        if text_of(&node, "kind")? == kind && text_of(&node, "name")? == name {
            found = Some(node);
        }
    }
    Ok(found)
}

fn keep_checks_only<'py>(
    plans: Vec<Bound<'py, PyAny>>,
    selection: &SelectionFacts,
) -> PyResult<Vec<Bound<'py, PyAny>>> {
    if !selection.check_only() {
        return Ok(plans);
    }
    let mut kept: Vec<Bound<'py, PyAny>> = Vec::new();
    for planned in plans {
        if planned.get_item(0)?.extract::<String>()? == CHECK {
            kept.push(planned);
        }
    }
    Ok(kept)
}

fn completed(py: Python<'_>, status: &str) -> PyResult<bool> {
    constant_contains(py, (ThirdPartyHost::CompletedExecutionStatuses, status))
}

/// Dagster events for an execution JSON document.
#[pyfunction]
#[pyo3(signature = (**options))]
pub(crate) fn dagster_payload_results<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<Vec<Bound<'py, PyAny>>> {
    let arguments: KeywordArguments<'py> = KeywordArguments::read(
        py,
        "dagster_payload_results",
        options,
        (
            &[
                "dag",
                "payload",
                "command",
                "selected_asset_paths",
                "selected_check_keys",
                "check_selection_is_explicit",
            ],
            &[],
        ),
    )?;
    let dag: Bound<'py, PyAny> = arguments.value("dag")?;
    let payload: Bound<'py, PyAny> = arguments.value("payload")?;
    let command: Vec<String> = arguments.value("command")?.extract()?;
    let selection = SelectionFacts::from_arguments(&arguments)?;
    let clone_command: String = constant(py, ThirdPartyHost::CloneCommand)?.extract()?;
    let success: String = constant(py, ThirdPartyHost::SuccessExecutionStatus)?.extract()?;
    let source_kind: String = constant(py, ThirdPartyHost::SourceNodeKind)?.extract()?;
    let is_clone: bool = text_of(&payload, "command")? == clone_command;
    let selected_paths: HashSet<AssetPath> = if is_clone {
        HashSet::new()
    } else {
        selection.paths.clone()
    };
    let nodes = nodes_by_id(&dag)?;
    let mut asset_results: Vec<(String, Bound<'py, PyAny>)> = Vec::new();
    let mut record = |id: String, facts: Bound<'py, PyAny>| match asset_results
        .iter_mut()
        .find(|(listed, _)| *listed == id)
    {
        Some(entry) => entry.1 = facts,
        None => asset_results.push((id, facts)),
    };
    for payload_asset in items(&payload, "assets")? {
        let status: String = text_of(&payload_asset, "status")?;
        if !completed(py, &status)? || (is_clone && status != success) {
            continue;
        }
        let has_loader: bool = !get(&payload_asset, "loader")?.is_none();
        let kind: String = if has_loader {
            source_kind.clone()
        } else {
            text_of(&payload_asset, "kind")?
        };
        let Some(node) = node_by_kind_and_name(&dag, (&kind, &text_of(&payload_asset, "name")?))?
        else {
            continue;
        };
        record(text_of(&node, "id")?, payload_asset.clone());
        if text_of(&node, "kind")? == source_kind && has_loader {
            let loader_name: String = payload_asset.get_item("loader")?.str()?.extract()?;
            if let Some((loader_id, facts)) =
                loader_result(&dag, (&node, &payload_asset), &loader_name)?
            {
                record(loader_id, facts.into_any());
            }
        }
    }
    let mut plans: Vec<Bound<'py, PyAny>> = Vec::new();
    let kind: &str = if is_clone {
        MATERIALIZATION
    } else {
        MATERIALIZE
    };
    for node in sorted_nodes(&dag)? {
        let id: String = text_of(&node, "id")?;
        let Some((_, facts)) = asset_results.iter().find(|(listed, _)| *listed == id) else {
            continue;
        };
        if !asset_enabled(&node)? {
            continue;
        }
        let path: AssetPath = asset_path(&node)?;
        if !selected_paths.is_empty() && !selected_paths.contains(&path) {
            continue;
        }
        let metadata: Bound<'py, PyDict> = merged(
            &[
                (
                    "command",
                    PyString::new(py, &command_text(&command)).into_any(),
                ),
                ("sqlbuild_id", get(&node, "id")?),
            ],
            &metadata_from_mapping(facts)?,
        )?;
        plans.push(plan(py, (kind, &path), metadata, None)?);
    }
    let filter = CheckFilter {
        selection: &selection,
        selected_paths: &selected_paths,
        emitted_asset_paths: None,
    };
    let mut seen: HashSet<CheckKey> = HashSet::new();
    for check in items(&payload, "checks")? {
        let (check_results, updated) = check_plans((&dag, &nodes), &check, (&filter, seen))?;
        plans.extend(check_results);
        seen = updated;
    }
    keep_checks_only(plans, &selection)
}

fn envelope_fields<'py>(
    envelope: &Bound<'py, PyAny>,
    names: &[&str],
) -> PyResult<Vec<(String, Bound<'py, PyAny>)>> {
    names
        .iter()
        .map(|name| Ok(((*name).to_owned(), native_field(envelope, name)?)))
        .collect()
}

/// Dagster events for one live integration result record.
#[pyfunction]
#[pyo3(signature = (**options))]
pub(crate) fn dagster_integration_results<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<Vec<Bound<'py, PyAny>>> {
    let arguments: KeywordArguments<'py> = KeywordArguments::read(
        py,
        "dagster_integration_results",
        options,
        (
            &[
                "dag",
                "envelope",
                "command",
                "selected_asset_paths",
                "selected_check_keys",
                "check_selection_is_explicit",
                "emitted_asset_paths",
            ],
            &[],
        ),
    )?;
    let dag: Bound<'py, PyAny> = arguments.value("dag")?;
    let envelope: Bound<'py, PyAny> = arguments.value("envelope")?;
    let command: Vec<String> = arguments.value("command")?.extract()?;
    let emitted_asset_paths: Bound<'py, PyAny> = arguments.value("emitted_asset_paths")?;
    let selection = SelectionFacts::from_arguments(&arguments)?;
    let nodes = nodes_by_id(&dag)?;
    let source_kind: String = constant(py, ThirdPartyHost::SourceNodeKind)?.extract()?;
    let clone_command: String = constant(py, ThirdPartyHost::CloneCommand)?.extract()?;
    let envelope_command: String = native_field(&envelope, "command")?.str()?.extract()?;
    let mut plans: Vec<Bound<'py, PyAny>> = Vec::new();
    let asset: Bound<'py, PyAny> = native_field(&envelope, "asset")?;
    if !asset.is_none()
        && completed(
            py,
            &native_field(&asset, "status")?.str()?.extract::<String>()?,
        )?
    {
        let resource_id: String = native_field(&envelope, "resource_id")?.str()?.extract()?;
        let node: Option<&Bound<'py, PyAny>> = node_with_id(&nodes, &resource_id);
        let asset_facts: Bound<'py, PyAny> = asset.call_method0("as_dict")?;
        let mut projected: Vec<(String, Bound<'py, PyAny>)> = Vec::new();
        if let Some(node) = node {
            projected.push((text_of(node, "id")?, asset_facts.clone()));
            let loader: Bound<'py, PyAny> = native_field(&asset, "loader")?;
            let loader_facts: Option<(String, Bound<'py, PyDict>)> =
                if text_of(node, "kind")? == source_kind && !loader.is_none() {
                    loader_result(
                        &dag,
                        (node, &asset_facts),
                        &loader.str()?.extract::<String>()?,
                    )?
                } else {
                    None
                };
            if let Some((loader_id, facts)) = loader_facts
                && node_with_id(&nodes, &loader_id).is_some()
            {
                projected.push((loader_id, facts.into_any()));
            }
        }
        let kind: &str = if envelope_command == clone_command {
            MATERIALIZATION
        } else {
            MATERIALIZE
        };
        let fields = envelope_fields(
            &envelope,
            &[
                "event_id",
                "resource_attempt_id",
                "resource_id",
                "event_sequence",
                "duration_ms",
            ],
        )?;
        for candidate in sorted_nodes(&dag)? {
            let id: String = text_of(&candidate, "id")?;
            let Some((_, facts)) = projected.iter().rev().find(|(listed, _)| *listed == id) else {
                continue;
            };
            if !asset_enabled(&candidate)? {
                continue;
            }
            let path: AssetPath = asset_path(&candidate)?;
            if !selection.paths.is_empty() && !selection.paths.contains(&path) {
                continue;
            }
            let mut base: Vec<(&str, Bound<'py, PyAny>)> = vec![
                (
                    "command",
                    PyString::new(py, &command_text(&command)).into_any(),
                ),
                ("sqlbuild_id", get(&candidate, "id")?),
            ];
            for (name, value) in &fields {
                base.push((name.as_str(), value.clone()));
            }
            plans.push(plan(
                py,
                (kind, &path),
                merged(&base, &metadata_from_mapping(facts)?)?,
                None,
            )?);
        }
    }
    let standalone: bool = constant_contains(
        py,
        (ThirdPartyHost::StandaloneCheckCommands, &envelope_command),
    )?;
    let emitted: HashSet<AssetPath> = path_set(&emitted_asset_paths)?;
    let filter = CheckFilter {
        selection: &selection,
        selected_paths: &selection.paths,
        emitted_asset_paths: (!selection.check_only() && !standalone).then_some(&emitted),
    };
    let check_fields = envelope_fields(
        &envelope,
        &[
            "event_id",
            "resource_attempt_id",
            "resource_id",
            "event_sequence",
        ],
    )?;
    let mut seen: HashSet<CheckKey> = HashSet::new();
    for check in native_field(&envelope, "checks")?.try_iter()? {
        let facts: Bound<'py, PyDict> = check?.call_method0("as_dict")?.cast_into()?;
        let record: Bound<'py, PyDict> = facts.copy()?;
        for (name, value) in &check_fields {
            record.set_item(name, value)?;
        }
        let (check_results, updated) =
            check_plans((&dag, &nodes), record.as_any(), (&filter, seen))?;
        plans.extend(check_results);
        seen = updated;
    }
    keep_checks_only(plans, &selection)
}

/// Dagster events for selected materializable assets when no execution document exists.
#[pyfunction]
#[pyo3(signature = (*, dag, command, selected_asset_paths))]
pub(crate) fn dagster_selected_asset_results<'py>(
    dag: Bound<'py, PyAny>,
    command: Vec<String>,
    selected_asset_paths: Bound<'py, PyAny>,
) -> PyResult<Vec<Bound<'py, PyAny>>> {
    let py: Python<'py> = dag.py();
    let selected: HashSet<AssetPath> = path_set(&selected_asset_paths)?;
    let materializable: Bound<'py, PyAny> = constant(py, ThirdPartyHost::MaterializableNodeKinds)?;
    let mut plans: Vec<Bound<'py, PyAny>> = Vec::new();
    for node in sorted_nodes(&dag)? {
        if !selected.is_empty() && !selected.contains(&asset_path(&node)?) {
            continue;
        }
        if !asset_enabled(&node)? || !materializable.contains(text_of(&node, "kind")?)? {
            continue;
        }
        let path: AssetPath = asset_path(&node)?;
        let metadata: Bound<'py, PyDict> = PyDict::new(py);
        metadata.set_item("command", command_text(&command))?;
        metadata.set_item("sqlbuild_id", get(&node, "id")?)?;
        plans.push(plan(py, (MATERIALIZE, &path), metadata, None)?);
    }
    Ok(plans)
}

/// The version-1 execution document in `text`, or `None` when it is absent or not one.
#[pyfunction]
#[pyo3(signature = (*, text))]
pub(crate) fn dagster_execution_payload<'py>(
    text: Bound<'py, PyString>,
) -> PyResult<Option<Bound<'py, PyAny>>> {
    let py: Python<'py> = text.py();
    let stripped: Bound<'py, PyAny> = text.call_method0("strip")?;
    if !stripped.is_truthy()? {
        return Ok(None);
    }
    let payload: Bound<'py, PyAny> = match imported(py, "json", "loads")?.call1((stripped,)) {
        Ok(payload) => payload,
        Err(error) if error.is_instance_of::<PyValueError>(py) => {
            let decode: Bound<'py, PyAny> = imported(py, "json", "JSONDecodeError")?;
            if error.value(py).is_instance(&decode)? {
                return Ok(None);
            }
            return Err(error);
        }
        Err(error) => return Err(error),
    };
    if !payload.is_instance(&imported(py, "collections.abc", "Mapping")?)? {
        return Ok(None);
    }
    let version: Bound<'py, PyAny> = get(&payload, "version")?;
    if !version.eq(EXECUTION_DOCUMENT_VERSION.into_pyobject(py)?)? {
        return Ok(None);
    }
    Ok(Some(payload))
}
