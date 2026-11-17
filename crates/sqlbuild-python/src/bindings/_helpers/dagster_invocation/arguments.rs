//! SQLBuild CLI arguments for one Dagster invocation: selections, JSON output and event output.

use std::collections::HashSet;

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::{PyDict, PyDictMethods};
use sqlbuild_core::dagster_invocation::constants::{
    JSON_OUTPUT_FLAG, SCENARIO_TEST_ARGUMENT_COUNT,
};
use sqlbuild_core::dagster_invocation::main::caller_json_output_path::caller_json_output_path;
use sqlbuild_core::dagster_invocation::main::has_explicit_scenario_selector::has_explicit_scenario_selector;
use sqlbuild_core::dagster_invocation::main::json_output_requested::json_output_requested;
use sqlbuild_core::dagster_invocation::main::wrap_selectors::wrap_selectors;
use sqlbuild_host::entries::models::ThirdPartyHost;

use crate::bindings::_helpers::dagster_invocation::access::{
    AssetPath, CheckKey, asset_path, asset_path_or_empty, check_enabled, check_key_set, constant,
    constant_contains, get, items, node_with_id, nodes_by_id, path_set, string_tuple, text_of,
};
use crate::bindings::_helpers::dagster_invocation::results::{dagster_check_name, sorted_nodes};
use sqlbuild_python_base::boundary::main::imported::imported;
use sqlbuild_python_base::boundary::models::KeywordArguments;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

const SELECT_FILE_FLAG: &str = "--select-file";
const EVENT_OUTPUT_FLAG: &str = "--event-output";
const SCENARIO_TEST_COMMAND: [&str; 2] = ["scenario", "test"];
const SELECTOR_FILE: (&str, &str) = ("sqlbuild-dagster-select-", ".txt");
const EXECUTION_JSON_FILE: (&str, &str) = ("sqlbuild-dagster-execution-", ".json");
const EVENT_FILE: (&str, &str) = ("sqlbuild-dagster-events-", ".jsonl");
const LOAD_COMMAND: &str = "load";
const CHECK_COMMAND: &str = "check";
const SELECTOR_LOG_WIDTH: usize = 100;

/// A new, closed temporary file holding `lines`, as a `pathlib.Path`.
fn temporary_file<'py>(
    py: Python<'py>,
    (prefix, suffix): (&str, &str),
    lines: &[String],
) -> PyResult<Bound<'py, PyAny>> {
    let options: Bound<'py, PyDict> = PyDict::new(py);
    options.set_item("mode", "w")?;
    options.set_item("encoding", "utf-8")?;
    options.set_item("prefix", prefix)?;
    options.set_item("suffix", suffix)?;
    options.set_item("delete", false)?;
    let handle: Bound<'py, PyAny> =
        imported(py, "tempfile", "NamedTemporaryFile")?.call((), Some(&options))?;
    for line in lines {
        handle.call_method1("write", (format!("{line}\n"),))?;
    }
    handle.call_method0("close")?;
    imported(py, "pathlib", "Path")?.call1((native_field(&handle, "name")?,))
}

fn is_scenario_test(args: &[String]) -> bool {
    args.len() >= SCENARIO_TEST_ARGUMENT_COUNT
        && args[..SCENARIO_TEST_ARGUMENT_COUNT] == SCENARIO_TEST_COMMAND
}

fn check_is_selected(
    check: &Bound<'_, PyAny>,
    (check_name, nodes): (&str, &[(String, Bound<'_, PyAny>)]),
    selected_check_keys: &HashSet<CheckKey>,
) -> PyResult<bool> {
    for asset_id in items(check, "checked_asset_ids")? {
        let Some(node) = node_with_id(nodes, &asset_id.str()?.extract::<String>()?) else {
            continue;
        };
        if selected_check_keys.contains(&(asset_path_or_empty(node)?, check_name.to_owned())) {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Selectors of every enabled DAG check matching a selected check key, without duplicates.
fn selected_check_selectors(
    dag: &Bound<'_, PyAny>,
    selected_check_keys: &HashSet<CheckKey>,
) -> PyResult<Vec<String>> {
    let nodes = nodes_by_id(dag)?;
    let mut selectors: Vec<String> = Vec::new();
    for check in items(dag, "checks")? {
        if !check_enabled(&check)? {
            continue;
        }
        let name: String = dagster_check_name(&check)?;
        if check_is_selected(&check, (&name, &nodes), selected_check_keys)? {
            let selector: String = text_of(&check, "name")?;
            if !selectors.contains(&selector) {
                selectors.push(selector);
            }
        }
    }
    Ok(selectors)
}

/// Asset paths checked by every selected check.
fn selected_check_asset_paths(
    dag: &Bound<'_, PyAny>,
    selected_check_keys: &HashSet<CheckKey>,
) -> PyResult<HashSet<AssetPath>> {
    let nodes = nodes_by_id(dag)?;
    let mut paths: HashSet<AssetPath> = HashSet::new();
    for check in items(dag, "checks")? {
        if !check_enabled(&check)? {
            continue;
        }
        let name: String = dagster_check_name(&check)?;
        if !check_is_selected(&check, (&name, &nodes), selected_check_keys)? {
            continue;
        }
        for asset_id in items(&check, "checked_asset_ids")? {
            if let Some(node) = node_with_id(&nodes, &asset_id.str()?.extract::<String>()?) {
                paths.insert(asset_path_or_empty(node)?);
            }
        }
    }
    Ok(paths)
}

/// The selection appended to the arguments and the selector file holding it, if any.
struct Selection<'py> {
    args: Vec<String>,
    selectors: Vec<String>,
    selector_file: Option<Bound<'py, PyAny>>,
}

fn unselected<'py>(args: &[String]) -> Selection<'py> {
    Selection {
        args: args.to_vec(),
        selectors: Vec::new(),
        selector_file: None,
    }
}

fn with_selector_file<'py>(
    py: Python<'py>,
    args: &[String],
    selectors: Vec<String>,
) -> PyResult<Selection<'py>> {
    let file: Bound<'py, PyAny> = temporary_file(py, SELECTOR_FILE, &selectors)?;
    let mut selected: Vec<String> = args.to_vec();
    selected.push(SELECT_FILE_FLAG.to_owned());
    selected.push(file.str()?.extract()?);
    Ok(Selection {
        args: selected,
        selectors,
        selector_file: Some(file),
    })
}

fn scenario_selection<'py>(
    (args, dag): (&[String], &Bound<'py, PyAny>),
    read_selection: &Bound<'py, PyAny>,
) -> PyResult<Selection<'py>> {
    if has_explicit_scenario_selector(args) {
        return Ok(unselected(args));
    }
    let (selected_paths, selected_check_keys) = selection_facts(read_selection)?;
    let selected_check_keys: &HashSet<CheckKey> = &selected_check_keys;
    let selected_paths: HashSet<AssetPath> = if selected_paths.is_empty() {
        selected_check_keys
            .iter()
            .map(|(path, _)| path.clone())
            .collect()
    } else {
        selected_paths
    };
    if selected_paths.is_empty() && selected_check_keys.is_empty() {
        return Ok(unselected(args));
    }
    let nodes = nodes_by_id(dag)?;
    let mut selected_ids: HashSet<String> = HashSet::new();
    for node in items(dag, "nodes")? {
        if selected_paths.contains(&asset_path_or_empty(&node)?) {
            selected_ids.insert(text_of(&node, "id")?);
        }
    }
    let scenario_kind: String = constant(dag.py(), ThirdPartyHost::ScenarioCheckKind)?.extract()?;
    let mut selectors: Vec<String> = Vec::new();
    for check in items(dag, "checks")? {
        if !check_enabled(&check)? || text_of(&check, "kind")? != scenario_kind {
            continue;
        }
        let mut checked: bool = false;
        for asset_id in items(&check, "checked_asset_ids")? {
            checked |= selected_ids.contains(&asset_id.str()?.extract::<String>()?);
        }
        if !checked {
            continue;
        }
        let name: String = dagster_check_name(&check)?;
        if !selected_check_keys.is_empty()
            && !check_is_selected(&check, (&name, &nodes), selected_check_keys)?
        {
            continue;
        }
        let selector: Bound<'py, PyAny> = get(&check, "name")?;
        if !selector.is_none() {
            selectors.push(selector.str()?.extract()?);
        }
    }
    if selectors.is_empty() {
        return Ok(unselected(args));
    }
    let mut selected: Vec<String> = args.to_vec();
    selected.extend(selectors.iter().cloned());
    Ok(Selection {
        args: selected,
        selectors,
        selector_file: None,
    })
}

fn asset_selection<'py>(
    (args, dag): (&[String], &Bound<'py, PyAny>),
    read_selection: &Bound<'py, PyAny>,
) -> PyResult<Selection<'py>> {
    let py: Python<'py> = dag.py();
    if is_scenario_test(args) {
        return scenario_selection((args, dag), read_selection);
    }
    if !constant_contains(py, (ThirdPartyHost::AssetSelectionCommands, &args[0]))? {
        return Ok(unselected(args));
    }
    let explicit: Bound<'py, PyAny> = constant(py, ThirdPartyHost::ExplicitSelectionFlags)?;
    for argument in args {
        if explicit.contains(argument)? {
            return Ok(unselected(args));
        }
    }
    let (mut selected_paths, selected_check_keys) = selection_facts(read_selection)?;
    let selected_check_keys: &HashSet<CheckKey> = &selected_check_keys;
    if args[0] == CHECK_COMMAND && !selected_check_keys.is_empty() {
        let selectors: Vec<String> = selected_check_selectors(dag, selected_check_keys)?;
        if !selectors.is_empty() {
            return with_selector_file(py, args, selectors);
        }
    }
    if !selected_check_keys.is_empty() {
        selected_paths.extend(selected_check_asset_paths(dag, selected_check_keys)?);
    }
    if selected_paths.is_empty() {
        return Ok(unselected(args));
    }
    let kinds: Bound<'py, PyAny> = constant(
        py,
        if args[0] == LOAD_COMMAND {
            ThirdPartyHost::LoadSelectableNodeKinds
        } else {
            ThirdPartyHost::DefaultSelectableNodeKinds
        },
    )?;
    let mut selectors: Vec<String> = Vec::new();
    for node in sorted_nodes(dag)? {
        if !selected_paths.contains(&asset_path(&node)?) {
            continue;
        }
        if !kinds.contains(text_of(&node, "kind")?)? {
            continue;
        }
        let selector: Bound<'py, PyAny> = get(&node, "name")?;
        if !selector.is_none() {
            selectors.push(selector.str()?.extract()?);
        }
    }
    if selectors.is_empty() {
        return Ok(unselected(args));
    }
    with_selector_file(py, args, selectors)
}

/// The selected asset paths and check keys the Dagster context reports.
fn selection_facts(
    read_selection: &Bound<'_, PyAny>,
) -> PyResult<(HashSet<AssetPath>, HashSet<CheckKey>)> {
    let facts: Bound<'_, PyAny> = read_selection.call0()?;
    Ok((
        path_set(&facts.get_item(0)?)?,
        check_key_set(&facts.get_item(1)?)?,
    ))
}

fn optional<'py>(py: Python<'py>, value: Option<Bound<'py, PyAny>>) -> Bound<'py, PyAny> {
    value.unwrap_or_else(|| py.None().into_bound(py))
}

/// The CLI arguments, Dagster selection, selector file and output files of one invocation.
#[pyfunction]
#[pyo3(signature = (**options))]
pub(crate) fn dagster_cli_arguments<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyDict>> {
    let arguments: KeywordArguments<'py> = KeywordArguments::read(
        py,
        "dagster_cli_arguments",
        options,
        (&["args", "dag", "has_context", "read_selection"], &[]),
    )?;
    let args: Vec<String> = arguments.value("args")?.extract()?;
    let dag: Option<Bound<'py, PyAny>> = Some(arguments.value("dag")?);
    let has_context: bool = arguments.flag("has_context")?;
    let read_selection: Bound<'py, PyAny> = arguments.value("read_selection")?;
    let managed_dag: Option<Bound<'py, PyAny>> =
        dag.filter(|dag| !dag.is_none() && has_context && !args.is_empty());
    let selection: Selection<'py> = match &managed_dag {
        Some(dag) => asset_selection((&args, dag), &read_selection)?,
        None => unselected(&args),
    };
    let managed: bool = managed_dag.is_some();
    let mut resolved: Vec<String> = selection.args.clone();
    let mut json_path: Option<Bound<'py, PyAny>> = None;
    if managed
        && !json_output_requested(&resolved)
        && (constant_contains(py, (ThirdPartyHost::AssetSelectionCommands, &resolved[0]))?
            || is_scenario_test(&resolved))
    {
        let path: Bound<'py, PyAny> = temporary_file(py, EXECUTION_JSON_FILE, &[])?;
        resolved.push(JSON_OUTPUT_FLAG.to_owned());
        resolved.push(path.str()?.extract()?);
        json_path = Some(path);
    }
    let json_owned: bool = json_path.is_some();
    if json_path.is_none()
        && let Some(path) = caller_json_output_path(&resolved)
    {
        json_path = Some(imported(py, "pathlib", "Path")?.call1((path,))?);
    }
    let event_path: Option<Bound<'py, PyAny>> = if managed
        && constant_contains(py, (ThirdPartyHost::LiveEventCommands, &resolved[0]))?
        && !resolved
            .iter()
            .any(|argument| argument == EVENT_OUTPUT_FLAG)
    {
        Some(temporary_file(py, EVENT_FILE, &[])?)
    } else {
        None
    };
    let result: Bound<'py, PyDict> = PyDict::new(py);
    result.set_item("args", string_tuple(py, &resolved)?)?;
    result.set_item("selection", string_tuple(py, &selection.selectors)?)?;
    result.set_item("selector_file", optional(py, selection.selector_file))?;
    result.set_item("execution_json_path", optional(py, json_path))?;
    result.set_item("execution_json_owned", json_owned)?;
    result.set_item("event_jsonl_path", optional(py, event_path))?;
    Ok(result)
}

/// Selectors packed into log lines of at most 100 characters.
#[pyfunction]
#[pyo3(signature = (*, selectors))]
pub(crate) fn dagster_selector_log_lines(selectors: Vec<String>) -> Vec<String> {
    wrap_selectors(&selectors, SELECTOR_LOG_WIDTH)
}
