//! `sqb freshness`: observe selected source freshness without writing state.

use std::collections::BTreeSet;

use pyo3::exceptions::PyValueError;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyErr, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods, PyString, PyTuple};
use sqlbuild_core::cli_text::models::CliStyle;
use sqlbuild_core::freshness_text::main::freshness_exit_code::freshness_exit_code;
use sqlbuild_core::freshness_text::main::freshness_json::freshness_json;
use sqlbuild_core::freshness_text::main::render_freshness_text::render_freshness_text;
use sqlbuild_core::freshness_text::models::{FreshnessSourceResult, FreshnessStatus};

use sqlbuild_host::entries::models::AdapterHost;
use sqlbuild_orchestration::runs::models::NodeOutcome;

use crate::bindings::_helpers::command_runtime::adapters::resolve_cli_adapter;
use crate::bindings::_helpers::command_runtime::connection::resolve_project_connection_config;
use crate::bindings::_helpers::compile_driver::discovery::discover_project_inputs;
use crate::bindings::_helpers::compile_driver::project::build_project_graph;
use crate::bindings::_helpers::compiled_project::snapshot_execution::VerifiedRun;
use crate::bindings::_helpers::contracts::spec_resolution::resolve_effective_adapter_name;
use crate::bindings::_helpers::freshness::freshness_observe::{PreviousState, observe_freshness};
use crate::bindings::_helpers::freshness::observation::Observer;
use crate::bindings::_helpers::freshness::state_io::{StateRead, read_latest};
use crate::bindings::_helpers::native_host::connections::connection_failure;
use crate::bindings::_helpers::planner_selection::selectors::{
    expand_project_upstream_keys, select_project_graph,
};
use crate::bindings::_helpers::planner_selection::unit_tests::reject_unit_test_selectors;
use crate::bindings::_helpers::presentation_text::terminal::stdout_supports_color;
use crate::bindings::_helpers::records::construct::enum_member;
use crate::bindings::_helpers::records::ids::RecordId;
use crate::bindings::_helpers::result_output::documents::write_execution_json_output;

use crate::bindings::_helpers::native_host::host;
use crate::bindings::_helpers::records::construct::error_class;
use crate::bindings::_helpers::records::fields::field as native_field;
use crate::bindings::_helpers::records::ids::ErrorId;
use sqlbuild_core::constants::SOURCE_FRESHNESS_TABLE_NAME as STATE_TABLE_NAME;
const STALE_REQUIRES_STATE_MESSAGE: &str = "freshness --fail-on-stale requires --state";
const STALE_REQUIRES_STATE_CODE: &str = "C238";

fn imported<'py>(py: Python<'py>, module: &str, name: &str) -> PyResult<Bound<'py, PyAny>> {
    py.import(module)?.getattr(name)
}

fn keywords<'py>(
    py: Python<'py>,
    items: &[(&str, &Bound<'py, PyAny>)],
) -> PyResult<Bound<'py, PyDict>> {
    let dict: Bound<'py, PyDict> = PyDict::new(py);
    for (name, value) in items {
        dict.set_item(*name, value)?;
    }
    Ok(dict)
}

fn user_error(py: Python<'_>, (message, code): (&str, &str)) -> PyErr {
    let raised = (|| {
        let options: Bound<'_, PyDict> = PyDict::new(py);
        options.set_item("code", code)?;
        error_class(py, ErrorId::CliUser)?.call((message,), Some(&options))
    })();
    match raised {
        Ok(error) => PyErr::from_value(error),
        Err(error) => error,
    }
}

fn write(py: Python<'_>, text: &str) -> PyResult<()> {
    py.import("sys")?
        .getattr("stdout")?
        .call_method1("write", (text,))?;
    Ok(())
}

/// Names of the sources among `keys` and their upstream closures.
fn source_names_for_keys(
    graph: &Bound<'_, PyAny>,
    keys: &Bound<'_, PyAny>,
) -> PyResult<BTreeSet<String>> {
    let mut names: BTreeSet<String> = BTreeSet::new();
    let py: Python<'_> = graph.py();
    let source_type: Bound<'_, PyAny> = enum_member(py, RecordId::CompiledResourceType, "SOURCE")?;
    let upstream_deps: Bound<'_, PyAny> = native_field(graph, "upstream_deps")?;
    for key in keys.try_iter()? {
        let key: Bound<'_, PyAny> = key?;
        let upstream: Bound<'_, PyAny> =
            expand_project_upstream_keys(key.clone(), &upstream_deps)?.into_any();
        for candidate in std::iter::once(Ok(key.clone())).chain(upstream.try_iter()?) {
            let candidate: Bound<'_, PyAny> = candidate?;
            if native_field(&candidate, "resource_type")?.eq(&source_type)? {
                names.insert(native_field(&candidate, "name")?.extract()?);
            }
        }
    }
    Ok(names)
}

fn graph_keys<'py>(
    graph: &Bound<'py, PyAny>,
    selectors: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = graph.py();
    let empty: Bound<'py, PyAny> = PyTuple::empty(py).into_any();
    Ok(select_project_graph(graph, selectors, &empty)?.into_any())
}

/// Selected source names, sorted: selected or all keys and their upstream sources, minus excluded.
fn selected_source_names(
    graph: &Bound<'_, PyAny>,
    (select, exclude): (&Bound<'_, PyAny>, &Bound<'_, PyAny>),
) -> PyResult<Vec<String>> {
    let selected_keys: Bound<'_, PyAny> = if select.is_truthy()? {
        graph_keys(graph, select)?
    } else {
        native_field(graph, "all_keys")?.call_method0("values")?
    };
    let selected: BTreeSet<String> = source_names_for_keys(graph, &selected_keys)?;
    let excluded: BTreeSet<String> = if exclude.is_truthy()? {
        source_names_for_keys(graph, &graph_keys(graph, exclude)?)?
    } else {
        BTreeSet::new()
    };
    Ok(selected.difference(&excluded).cloned().collect())
}

/// Latest direct freshness state records, by identity, across the project's target schemas.
fn previous_records<'py>(
    adapter: &Bound<'py, PyAny>,
    connection: &Bound<'py, PyAny>,
    project: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyDict>> {
    let py: Python<'py> = adapter.py();
    let mut database: Option<Bound<'py, PyAny>> = None;
    let mut schemas: BTreeSet<String> = BTreeSet::new();
    for collection in ["models", "seeds", "functions"] {
        for item in native_field(project, collection)?.try_iter()? {
            let destination: Bound<'py, PyAny> = native_field(&item?, "destination")?;
            let destination_database: Bound<'py, PyAny> = native_field(&destination, "database")?;
            if database.is_none() && !destination_database.is_none() {
                database = Some(destination_database);
            }
            let schema: Bound<'py, PyAny> = native_field(&destination, "schema")?;
            if !schema.is_none() {
                schemas.insert(schema.extract()?);
            }
        }
    }
    let database: Bound<'py, PyAny> = database.unwrap_or_else(|| py.None().into_bound(py));
    let locations: Vec<Bound<'py, PyTuple>> = schemas
        .iter()
        .map(|schema| {
            PyTuple::new(
                py,
                [
                    database.clone(),
                    PyString::new(py, schema).into_any(),
                    PyString::new(py, STATE_TABLE_NAME).into_any(),
                ],
            )
        })
        .collect::<PyResult<_>>()?;
    let lookup: Bound<'py, PyAny> = host::entry(py, AdapterHost::BuildRelationLookup)?.call(
        (),
        Some(&keywords(
            py,
            &[
                ("adapter", adapter),
                ("connection", connection),
                ("locations", &PyTuple::new(py, locations)?.into_any()),
            ],
        )?),
    )?;
    let read: StateRead<'py> = StateRead {
        connection: connection.clone(),
        execute: native_field(adapter, "execute")?,
        render_qualified_name: native_field(adapter, "render_qualified_name")?,
        render_read_latest_sql: native_field(adapter, "render_read_latest_source_freshness_sql")?,
    };
    let records: Bound<'py, PyDict> = PyDict::new(py);
    for schema in &schemas {
        let exists: Bound<'py, PyDict> = keywords(py, &[("database", &database)])?;
        exists.set_item("schema", schema)?;
        exists.set_item("name", STATE_TABLE_NAME)?;
        let table_exists: bool = lookup
            .call_method("exists", (), Some(&exists))?
            .is_truthy()?;
        let schema: Bound<'py, PyAny> = PyString::new(py, schema).into_any();
        records.update(
            native_field(
                &read_latest(&read, (&database, &schema), table_exists, None)?,
                "records",
            )?
            .downcast()?,
        )?;
    }
    Ok(records)
}

/// Connect, read previous state when requested, observe, and close the connection.
fn observe(
    request: &Bound<'_, PyAny>,
    (discovered, project_dir): (&Bound<'_, PyAny>, &Bound<'_, PyAny>),
    (adapter, graph): (&Bound<'_, PyAny>, &Bound<'_, PyAny>),
    names: &[String],
) -> PyResult<Vec<FreshnessSourceResult>> {
    let py: Python<'_> = adapter.py();
    let connection_config: Bound<'_, PyAny> = resolve_project_connection_config(
        py,
        Some(keywords(
            py,
            &[
                ("discovered_inputs", discovered),
                ("project_dir", project_dir),
                (
                    "selected_target",
                    &native_field(request, "selected_target")?,
                ),
                ("cli_vars", &native_field(request, "cli_vars")?),
            ],
        )?),
    )?;
    let connection: Bound<'_, PyAny> = adapter
        .call_method1("connect", (connection_config,))
        .map_err(|error| connection_failure(adapter, &error))?;
    let observed = (|| {
        let project: Bound<'_, PyAny> = native_field(graph, "project")?;
        let by_identity: Option<Bound<'_, PyAny>> =
            if native_field(request, "compare_state")?.extract::<bool>()? {
                Some(previous_records(adapter, &connection, &project)?.into_any())
            } else {
                None
            };
        let sources: Vec<Bound<'_, PyAny>> = native_field(&project, "sources")?
            .try_iter()?
            .map(|source| native_field(&source?, "source_entry"))
            .collect::<PyResult<_>>()?;
        let observed_at: Bound<'_, PyAny> = imported(py, "datetime", "datetime")?
            .call_method1("now", (imported(py, "datetime", "UTC")?,))?;
        let observer: Observer<'_> = Observer {
            adapter: adapter.clone(),
            connection: connection.clone(),
            observed_at,
        };
        let state: PreviousState<'_> = PreviousState { by_identity };
        observe_freshness(
            &observer,
            (&PyTuple::new(py, sources)?.into_any(), names, &[]),
            (native_field(&project, "run_id")?.extract()?, &state),
        )
    })();
    let closed: PyResult<Bound<'_, PyAny>> = adapter.call_method1("close", (&connection,));
    let results: Vec<FreshnessSourceResult> = observed?;
    closed?;
    Ok(results)
}

/// An unreadable source or an age policy error fails; an unknown or warning age warns.
fn source_outcome(result: &FreshnessSourceResult) -> NodeOutcome {
    match (result.status, result.age_status.as_deref()) {
        (FreshnessStatus::Error, _) | (_, Some("error")) => NodeOutcome::Failed,
        (FreshnessStatus::Unknown, _) | (_, Some("warn" | "unknown")) => NodeOutcome::Warn,
        _ => NodeOutcome::Success,
    }
}

fn write_json_output(
    py: Python<'_>,
    payload: &str,
    (json_output, json_output_path): (bool, &Bound<'_, PyAny>),
) -> PyResult<()> {
    write_execution_json_output(
        py,
        PyString::new(py, &format!("{payload}\n")).into_any(),
        json_output,
        json_output_path.clone(),
    )
}

/// Run one `sqb freshness` request; a claimed run observes its verified sources and reports to its worker.
pub(crate) fn run_freshness_command<'py>(
    py: Python<'py>,
    request: &Bound<'py, PyAny>,
    verified: Option<VerifiedRun<'_, 'py>>,
) -> PyResult<i32> {
    let (worker, verified_inputs) = verified.unzip();
    let fail_on_stale: bool = native_field(request, "fail_on_stale")?.extract()?;
    let fail_on_error: bool = native_field(request, "fail_on_error")?.extract()?;
    if fail_on_stale && !native_field(request, "compare_state")?.extract::<bool>()? {
        return Err(user_error(
            py,
            (STALE_REQUIRES_STATE_MESSAGE, STALE_REQUIRES_STATE_CODE),
        ));
    }
    let project_dir: Bound<'_, PyAny> = match native_field(request, "project_dir")? {
        directory if !directory.is_none() => directory,
        _ => py.import("pathlib")?.getattr("Path")?.call_method0("cwd")?,
    };
    let discovered: Bound<'_, PyAny> = match verified_inputs {
        Some(discovered) => discovered.clone(),
        None => discover_project_inputs(py, Some(keywords(py, &[("project_dir", &project_dir)])?))?,
    };
    let select: Bound<'_, PyAny> = native_field(request, "select")?;
    let exclude: Bound<'_, PyAny> = native_field(request, "exclude")?;
    reject_unit_test_selectors(&select, &exclude, &discovered)?;
    let adapter_name: Bound<'_, PyAny> = resolve_effective_adapter_name(
        &native_field(&discovered, "project_config")?,
        &native_field(&discovered, "local_config")?,
    )?;
    let adapter: Bound<'_, PyAny> = resolve_cli_adapter(&adapter_name, &project_dir)?;
    let graph: Bound<'_, PyAny> = build_project_graph(
        py,
        Some(keywords(
            py,
            &[
                ("discovered_inputs", &discovered),
                ("adapter", &adapter),
                (
                    "selected_target",
                    &native_field(request, "selected_target")?,
                ),
                (
                    "no_sql_validation",
                    &native_field(request, "no_sql_validation")?,
                ),
                ("cli_vars", &native_field(request, "cli_vars")?),
            ],
        )?),
    )?;
    let names: Vec<String> = selected_source_names(&graph, (&select, &exclude))?;
    if let Some(worker) = &worker {
        for name in &names {
            worker.start_key(py, &format!("source:{name}"))?;
        }
    }
    let results: Vec<FreshnessSourceResult> = if names.is_empty() {
        Vec::new()
    } else {
        observe(
            request,
            (&discovered, &project_dir),
            (&adapter, &graph),
            &names,
        )?
    };
    if let Some(worker) = &worker {
        for result in &results {
            worker.finish_key(
                py,
                &format!("source:{}", result.name),
                source_outcome(result),
            )?;
        }
    }
    let payload: String =
        freshness_json(&results).map_err(|error| PyValueError::new_err(format!("{error:?}")))?;
    let json_output_path: Bound<'_, PyAny> = native_field(request, "json_output_path")?;
    if native_field(request, "json_output")?.extract::<bool>()? {
        write_json_output(py, &payload, (true, &json_output_path))?;
    } else {
        if !json_output_path.is_none() {
            write_json_output(py, &payload, (false, &json_output_path))?;
        }
        let use_color: bool =
            !native_field(request, "no_color")?.extract::<bool>()? && stdout_supports_color(py)?;
        write(py, &render_freshness_text(&results, CliStyle { use_color }))?;
    }
    Ok(freshness_exit_code(
        &results,
        (fail_on_stale, fail_on_error),
    ))
}
