//! Raw-query `sqb diff`: preparation, artifact materialization, comparison and cleanup.

use std::time::Instant;

use pyo3::exceptions::PyException;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyErr, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods, PyTuple};
use sqlbuild_cli::diff_report::errors::DiffUserError;
use sqlbuild_cli::diff_report::main::effective_max_examples::effective_max_examples;
use sqlbuild_cli::diff_report::main::query_column_problem::query_column_problem;
use sqlbuild_cli::diff_report::main::read_only_query_problem::read_only_query_problem;
use sqlbuild_cli::diff_report::models::QueryShapeFacts;
use sqlbuild_cli::diff_report::types::PhaseSeconds;

use sqlbuild_host::entries::models::{AdapterHost, ExecutorHost};

use crate::bindings::_helpers::boundary::panics::value_error;
use crate::bindings::_helpers::boundary::python_calls::{imported, keywords};
use crate::bindings::_helpers::command_runtime::adapters::resolve_cli_adapter;
use crate::bindings::_helpers::command_runtime::connection::resolve_project_connection_config;
use crate::bindings::_helpers::compile_attachment::effective_config::build_effective_runtime_config;
use crate::bindings::_helpers::compile_attachment::target::build_effective_target_namespace;
use crate::bindings::_helpers::contracts::spec_resolution::resolve_effective_adapter_name;
use crate::bindings::_helpers::diff_report::support::{
    call, cli_error, error_text, none, object, progress_line,
};
use crate::bindings::_helpers::diff_report::tolerances::cli_tolerance_overrides;
use crate::bindings::_helpers::native_host::host;
use crate::bindings::_helpers::records::construct::record_class;
use crate::bindings::_helpers::records::fields::field as native_field;
use crate::bindings::_helpers::records::fields::mapping_get;
use crate::bindings::_helpers::records::ids::{ErrorId, RecordId};
use crate::bindings::types::CompilerDetach;
use sqlbuild_analysis::diff_selection::main::raw_query_shape::raw_query_shape;
use sqlbuild_analysis::diff_selection::models::RawQueryShape;

const GENERIC_DIALECT: &str = "generic";
pub(crate) const NAMESPACELESS_ADAPTERS: [&str; 2] = ["duckdb", "motherduck"];

fn failure(py: Python<'_>, message: String, code: &str, help: Option<&str>) -> PyErr {
    cli_error(
        py,
        ErrorId::CliUser,
        &DiffUserError {
            message,
            code: code.to_owned(),
            help: help.map(str::to_owned),
        },
    )
}

fn user_failure(py: Python<'_>, error: DiffUserError) -> PyErr {
    cli_error(py, ErrorId::CliUser, &error)
}

/// The adapter of the effective adapter name, resolved for the project directory.
pub(crate) fn project_adapter<'py>(
    discovered: &Bound<'py, PyAny>,
    project_dir: &Bound<'py, PyAny>,
) -> PyResult<(String, Bound<'py, PyAny>)> {
    let name: Bound<'py, PyAny> = resolve_effective_adapter_name(
        &native_field(discovered, "project_config")?,
        &native_field(discovered, "local_config")?,
    )?;
    let adapter: Bound<'py, PyAny> = resolve_cli_adapter(&name, project_dir)?;
    Ok((name.extract()?, adapter))
}

fn query_input(
    py: Python<'_>,
    inline: Bound<'_, PyAny>,
    file: Bound<'_, PyAny>,
    label: &str,
) -> PyResult<String> {
    if !inline.is_none() {
        return inline.extract();
    }
    if file.is_none() {
        return Err(failure(
            py,
            format!("raw-query diff requires a {label} query"),
            "C229",
            None,
        ));
    }
    if !file.call_method0("exists")?.is_truthy()? || !file.call_method0("is_file")?.is_truthy()? {
        return Err(failure(
            py,
            format!("{label} query file does not exist: {}", file.str()?),
            "C230",
            None,
        ));
    }
    let keywords: Bound<'_, PyDict> = PyDict::new(py);
    keywords.set_item("encoding", "utf-8")?;
    match file.call_method("read_text", (), Some(&keywords)) {
        Ok(text) => text.extract(),
        Err(error)
            if error.is_instance_of::<pyo3::exceptions::PyOSError>(py)
                || error.is_instance_of::<pyo3::exceptions::PyUnicodeError>(py) =>
        {
            let replacement: PyErr = failure(
                py,
                format!(
                    "{label} query file could not be read as UTF-8: {}",
                    file.str()?
                ),
                "C231",
                None,
            );
            replacement.set_cause(py, Some(error));
            Err(replacement)
        }
        Err(error) => Err(error),
    }
}

fn read_only_query(
    py: Python<'_>,
    sql: &str,
    label: &str,
    adapter: &Bound<'_, PyAny>,
) -> PyResult<String> {
    let mut normalized: &str = sql.trim();
    if let Some(stripped) = normalized.strip_suffix(';') {
        normalized = stripped.trim_end();
    }
    if normalized.is_empty() {
        return Err(failure(
            py,
            format!("{label} query must not be empty"),
            "C232",
            None,
        ));
    }
    let dialect: Bound<'_, PyAny> = adapter.call_method0("sql_analysis_dialect")?;
    let dialect: String = if dialect.is_truthy()? {
        dialect.extract()?
    } else {
        GENERIC_DIALECT.to_owned()
    };
    let shape: RawQueryShape = py
        .compiler_detach(|| Ok::<_, String>(raw_query_shape(normalized, &dialect)))
        .map_err(value_error)?
        .map_err(|message| {
            failure(
                py,
                format!("{label} query is not one valid SQL statement: {message}"),
                "C234",
                None,
            )
        })?;
    read_only_query_problem(
        label,
        &QueryShapeFacts {
            root_key: shape.root_key,
            node_keys: shape.node_keys,
            writes_or_locks: shape.writes_or_locks,
        },
    )
    .map_err(|error| user_failure(py, error))?;
    Ok(normalized.to_owned())
}

positional_record! {
    extract;
    /// The target name, target, database and schema a raw-query diff resolves.
    struct ResolvedNamespace<'py> {
        target_name: Bound<'py, PyAny>,
        _target: Bound<'py, PyAny>,
        database: Bound<'py, PyAny>,
        schema: Bound<'py, PyAny>,
    }
}

/// Resolve raw SQL, the active target namespace, the adapter and its connection.
pub(crate) fn prepare_query_diff<'py>(
    request: &Bound<'py, PyAny>,
    (project_dir, discovered): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = request.py();
    let (adapter_name, adapter) = project_adapter(discovered, project_dir)?;
    let selected_target: Bound<'py, PyAny> = native_field(request, "selected_target")?;
    let cli_vars: Bound<'py, PyAny> = native_field(request, "cli_vars")?;
    let config: Bound<'py, PyAny> = resolve_project_connection_config(
        py,
        Some(keywords(
            py,
            &[
                ("discovered_inputs", discovered.clone()),
                ("project_dir", project_dir.clone()),
                ("selected_target", selected_target.clone()),
                ("cli_vars", cli_vars.clone()),
            ],
        )?),
    )?;
    let database: Bound<'py, PyAny> = if NAMESPACELESS_ADAPTERS.contains(&adapter_name.as_str()) {
        none(py)
    } else {
        mapping_get(&config, "database")?
    };
    let namespace: Bound<'py, PyAny> = build_effective_target_namespace(
        py,
        Some(keywords(
            py,
            &[
                ("discovered_inputs", discovered.clone()),
                ("selected_target", selected_target.clone()),
                ("connection_database", database.clone()),
                ("connection_schema", mapping_get(&config, "schema")?),
                (
                    "default_database",
                    adapter.call_method0("default_database")?,
                ),
                ("default_schema", adapter.call_method0("default_schema")?),
            ],
        )?),
    )
    .and_then(|value| pyo3::IntoPyObjectExt::into_bound_py_any(value, py))?;
    let ResolvedNamespace {
        target_name,
        database,
        schema,
        ..
    }: ResolvedNamespace<'py> = namespace.extract()?;
    if schema.is_none() {
        return Err(failure(
            py,
            "raw-query diff requires an active target schema".to_owned(),
            "C227",
            Some("configure the selected target schema before running the comparison"),
        ));
    }
    let left_sql: String = query_input(
        py,
        native_field(request, "left_query")?,
        native_field(request, "left_query_file")?,
        "left",
    )?;
    let right_sql: String = query_input(
        py,
        native_field(request, "right_query")?,
        native_field(request, "right_query_file")?,
        "right",
    )?;
    let left_sql: String = read_only_query(py, &left_sql, "left", &adapter)?;
    let right_sql: String = read_only_query(py, &right_sql, "right", &adapter)?;
    let runtime: Bound<'py, PyTuple> = build_effective_runtime_config(
        discovered,
        Some(selected_target.clone()).filter(|value| !value.is_none()),
        Some(cli_vars.clone()).filter(|value| !value.is_none()),
    )
    .and_then(|value| pyo3::IntoPyObjectExt::into_bound_py_any(value, py))?
    .downcast_into()?;
    let label = |name: &str, default: &str| -> PyResult<String> {
        let value: Bound<'py, PyAny> = native_field(request, name)?;
        let text: String = if value.is_truthy()? {
            value.extract()?
        } else {
            default.to_owned()
        };
        Ok(text.trim().to_owned())
    };
    let verbose: bool = native_field(request, "verbose")?.is_truthy()?;
    let examples = |name: &str| -> PyResult<Bound<'py, PyAny>> {
        object(
            py,
            effective_max_examples(native_field(request, name)?.extract()?, verbose),
        )
    };
    call(
        &record_class(py, RecordId::QueryDiffPreparation)?,
        &[
            ("adapter", &adapter),
            ("connection_config", &config),
            ("left_sql", &object(py, left_sql)?),
            ("right_sql", &object(py, right_sql)?),
            (
                "left_label",
                &object(py, label("left_label", "left query")?)?,
            ),
            (
                "right_label",
                &object(py, label("right_label", "right query")?)?,
            ),
            ("selected_target", &target_name),
            ("database", &database),
            ("schema", &schema),
            ("run_id", &runtime.get_item(2)?),
            (
                "artifact_ttl",
                &native_field(discovered, "project_config")?
                    .getattr("diff")?
                    .getattr("query_artifact_ttl")?,
            ),
            (
                "effective_max_column_examples",
                &examples("max_column_examples")?,
            ),
            (
                "effective_max_row_only_examples",
                &examples("max_row_only_examples")?,
            ),
        ],
    )
}

fn seconds(started: Instant) -> f64 {
    started.elapsed().as_secs_f64()
}

/// `RowDiffSamplingOverride` and the comparison options a request asks for.
pub(crate) fn execution_options<'py>(
    request: &Bound<'py, PyAny>,
    extra: &[(&str, &Bound<'py, PyAny>)],
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = request.py();
    let attribute = |name: &str| native_field(request, name);
    let sampling: Bound<'py, PyAny> = call(
        &host::entry(py, ExecutorHost::RowDiffSamplingOverride)?,
        &[
            ("row_limit", &attribute("sample_rows")?),
            ("seed", &attribute("sample_seed")?),
            ("exhaustive", &attribute("exhaustive")?),
        ],
    )?;
    let tolerances: Bound<'py, PyAny> = cli_tolerance_overrides(
        py,
        &attribute("tolerance_overrides")?.extract::<Vec<String>>()?,
    )?;
    let schema_only: Bound<'py, PyAny> = attribute("schema_only")?;
    let collect: Bound<'py, PyAny> = object(py, !schema_only.is_truthy()?)?;
    let mut pairs: Vec<(&str, &Bound<'py, PyAny>)> =
        vec![("schema_only", &schema_only), ("collect_samples", &collect)];
    let columns: Bound<'py, PyAny> = attribute("max_columns")?;
    let key: Bound<'py, PyAny> = attribute("unique_key_override")?;
    let unkeyed: Bound<'py, PyAny> = attribute("unkeyed")?;
    let excluded: Bound<'py, PyAny> = attribute("excluded_columns_override")?;
    pairs.extend([
        ("max_columns", &columns),
        ("sampling_override", &sampling),
        ("unique_key_override", &key),
        ("unkeyed", &unkeyed),
        ("excluded_columns_override", &excluded),
        ("tolerance_overrides", &tolerances),
    ]);
    pairs.extend_from_slice(extra);
    call(
        &host::entry(py, ExecutorHost::DiffExecutionOptions)?,
        &pairs,
    )
}

/// Materialize, compare and clean one raw-query pair; the result and per-phase seconds.
pub(crate) fn execute_query_diff<'py>(
    request: &Bound<'py, PyAny>,
    preparation: &Bound<'py, PyAny>,
) -> PyResult<(Bound<'py, PyAny>, PhaseSeconds)> {
    let py: Python<'py> = request.py();
    let adapter: Bound<'py, PyAny> = native_field(preparation, "adapter")?;
    let total_started: Instant = Instant::now();
    let mut phases: Vec<(String, f64)> = Vec::new();
    progress_line(py, "Query diff connection  START")?;
    let connection_started: Instant = Instant::now();
    let connection: Bound<'py, PyAny> = match adapter.call_method1(
        "connect",
        (native_field(preparation, "connection_config")?,),
    ) {
        Ok(connection) => connection,
        Err(error) => {
            progress_line(
                py,
                &format!(
                    "Query diff connection  ERROR  ({:.2}s)",
                    seconds(connection_started)
                ),
            )?;
            progress_line(
                py,
                &format!("Query diff total  ERROR  ({:.2}s)", seconds(total_started)),
            )?;
            return Err(error);
        }
    };
    phases.push(("connection".to_owned(), seconds(connection_started)));
    progress_line(
        py,
        &format!("Query diff connection  OK  ({:.2}s)", phases[0].1),
    )?;
    let lifecycle: Bound<'py, PyAny> = host::entry(py, ExecutorHost::QueryDiffArtifactLifecycle)?;
    let artifact = |side: &str| -> PyResult<Bound<'py, PyAny>> {
        call(
            &native_field(&lifecycle, "build")?,
            &[
                ("adapter", &adapter),
                ("run_id", &native_field(preparation, "run_id")?),
                ("side", &object(py, side)?),
                ("database", &native_field(preparation, "database")?),
                ("schema", &native_field(preparation, "schema")?),
            ],
        )
    };
    let left: Bound<'py, PyAny> = artifact("left")?;
    let right: Bound<'py, PyAny> = artifact("right")?;
    let mut created: Vec<Bound<'py, PyAny>> = Vec::new();
    let outcome: PyResult<Bound<'py, PyAny>> = compare_artifacts(
        (request, preparation),
        (&connection, &lifecycle),
        (&left, &right),
        (&mut created, &mut phases),
    );
    let cleanup_started: Instant = Instant::now();
    progress_line(py, "Query diff immediate cleanup  START")?;
    let mut cleanup_errors: Vec<PyErr> = Vec::new();
    for item in created.iter().rev() {
        if let Err(error) = call(
            &native_field(&lifecycle, "cleanup")?,
            &[
                ("adapter", &adapter),
                ("connection", &connection),
                ("artifact", item),
            ],
        ) {
            cleanup_errors.push(error);
        }
    }
    if let Err(error) = adapter.call_method1("close", (&connection,)) {
        cleanup_errors.push(error);
    }
    let cleanup_seconds: f64 = seconds(cleanup_started);
    if cleanup_errors.is_empty() {
        progress_line(
            py,
            &format!(
                "Query diff immediate cleanup  OK  ({} removed; {cleanup_seconds:.2}s)",
                created.len()
            ),
        )?;
    } else {
        progress_line(
            py,
            &format!(
                "Query diff immediate cleanup  ERROR  ({} failed; {cleanup_seconds:.2}s)",
                cleanup_errors.len()
            ),
        )?;
    }
    phases.push(("cleanup".to_owned(), cleanup_seconds));
    let total: f64 = seconds(total_started);
    phases.push(("total".to_owned(), total));
    let status: &str = if outcome.is_err() || !cleanup_errors.is_empty() {
        "ERROR"
    } else {
        "OK"
    };
    progress_line(py, &format!("Query diff total  {status}  ({total:.2}s)"))?;
    if !cleanup_errors.is_empty() {
        return Err(cleanup_errors.remove(0));
    }
    let model: Bound<'py, PyAny> = outcome?;
    if model.is_none() {
        return Err(cli_error(
            py,
            ErrorId::QueryDiffExecution,
            &DiffUserError {
                message: "query diff comparison completed without a result".to_owned(),
                code: "C256".to_owned(),
                help: None,
            },
        ));
    }
    let result: Bound<'py, PyAny> = call(
        &host::entry(py, ExecutorHost::DiffExecutionResult)?,
        &[("model_results", &PyTuple::new(py, [model])?.into_any())],
    )?;
    Ok((result, phases))
}

/// Reconcile, inspect, materialize both sides and compare them; the model result.
fn compare_artifacts<'py>(
    (request, preparation): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
    (connection, lifecycle): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
    (left, right): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
    (created, phases): (&mut Vec<Bound<'py, PyAny>>, &mut Vec<(String, f64)>),
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = request.py();
    let adapter: Bound<'py, PyAny> = native_field(preparation, "adapter")?;
    phases.push((
        "reconciliation".to_owned(),
        reconcile(preparation, connection, lifecycle)?,
    ));
    let (expires_at, inspection) = inspect(request, preparation, connection, (left, right))?;
    phases.push(("inspection".to_owned(), inspection));
    progress_line(py, "Query diff materialization  START")?;
    let started: Instant = Instant::now();
    for (artifact, sql) in [(left, "left_sql"), (right, "right_sql")] {
        if let Err(error) = call(
            &native_field(lifecycle, "materialize")?,
            &[
                ("adapter", &adapter),
                ("connection", connection),
                ("artifact", artifact),
                ("sql", &native_field(preparation, sql)?),
                ("expires_at", &expires_at),
            ],
        ) {
            progress_line(
                py,
                &format!(
                    "Query diff materialization  ERROR  ({:.2}s)",
                    seconds(started)
                ),
            )?;
            return Err(error);
        }
        created.push(artifact.clone());
    }
    let materialization: f64 = seconds(started);
    phases.push(("materialization".to_owned(), materialization));
    progress_line(
        py,
        &format!(
            "Query diff materialization  OK  ({}; expires {}; {materialization:.2}s)",
            native_field(preparation, "schema")?.str()?,
            expires_at.call_method0("isoformat")?.str()?
        ),
    )?;
    progress_line(py, "Query diff comparison  START")?;
    let started: Instant = Instant::now();
    let name: String = format!(
        "{} vs {}",
        native_field(preparation, "left_label")?.str()?,
        native_field(preparation, "right_label")?.str()?
    );
    let pairs: [(&str, Bound<'py, PyAny>); 3] = [
        (
            "max_column_examples",
            native_field(preparation, "effective_max_column_examples")?,
        ),
        (
            "max_row_only_examples",
            native_field(preparation, "effective_max_row_only_examples")?,
        ),
        ("comparison_name", object(py, name)?),
    ];
    let extra: Vec<(&str, &Bound<'py, PyAny>)> =
        pairs.iter().map(|(key, value)| (*key, value)).collect();
    let compared: PyResult<Bound<'py, PyAny>> =
        execution_options(request, &extra).and_then(|options| {
            call(
                &host::entry(py, ExecutorHost::ExecuteQueryDiff)?,
                &[
                    ("adapter", &adapter),
                    ("connection", connection),
                    ("left_relation", &native_field(left, "relation")?),
                    ("right_relation", &native_field(right, "relation")?),
                    ("options", &options),
                ],
            )
        });
    let comparison: f64 = seconds(started);
    phases.push(("comparison".to_owned(), comparison));
    match compared {
        Ok(model) => {
            progress_line(
                py,
                &format!("Query diff comparison  OK  ({comparison:.2}s)"),
            )?;
            Ok(model)
        }
        Err(error) => {
            progress_line(
                py,
                &format!("Query diff comparison  ERROR  ({comparison:.2}s)"),
            )?;
            Err(error)
        }
    }
}

fn reconcile(
    preparation: &Bound<'_, PyAny>,
    connection: &Bound<'_, PyAny>,
    lifecycle: &Bound<'_, PyAny>,
) -> PyResult<f64> {
    let py: Python<'_> = preparation.py();
    progress_line(py, "Query diff reconciliation  START")?;
    let started: Instant = Instant::now();
    let now: Bound<'_, PyAny> = imported(py, "datetime", "datetime")?
        .call_method1("now", (imported(py, "datetime", "UTC")?,))?;
    let cleaned: PyResult<Bound<'_, PyAny>> = call(
        &native_field(lifecycle, "cleanup_expired")?,
        &[
            ("adapter", &native_field(preparation, "adapter")?),
            ("connection", connection),
            ("database", &native_field(preparation, "database")?),
            ("schema", &native_field(preparation, "schema")?),
            ("now", &now),
        ],
    );
    let elapsed: f64 = seconds(started);
    let result: Bound<'_, PyAny> = match cleaned {
        Ok(result) => result,
        Err(error) if error.is_instance_of::<PyException>(py) => {
            progress_line(
                py,
                &format!(
                    "Query diff reconciliation  WARNING  recovery cleanup could not inspect historical ownership evidence: {} ({elapsed:.2}s)",
                    error_text(py, &error)?
                ),
            )?;
            return Ok(elapsed);
        }
        Err(error) => return Err(error),
    };
    progress_line(
        py,
        &format!(
            "Query diff reconciliation  OK  ({} expired; {elapsed:.2}s)",
            native_field(&result, "cleaned")?.len()?
        ),
    )?;
    for relation in native_field(&result, "untracked")?.try_iter()? {
        progress_line(
            py,
            &format!(
                "Query diff reconciliation  WARNING  reserved-name relation lacks matching ownership evidence: {}",
                native_field(&relation?, "name")?.str()?
            ),
        )?;
    }
    Ok(elapsed)
}

fn inspect<'py>(
    request: &Bound<'py, PyAny>,
    preparation: &Bound<'py, PyAny>,
    connection: &Bound<'py, PyAny>,
    artifacts: (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
) -> PyResult<(Bound<'py, PyAny>, f64)> {
    let py: Python<'py> = request.py();
    progress_line(py, "Query diff inspection  START")?;
    let started: Instant = Instant::now();
    match inspect_inputs(request, preparation, connection, artifacts) {
        Ok(ttl) => {
            let elapsed: f64 = seconds(started);
            progress_line(py, &format!("Query diff inspection  OK  ({elapsed:.2}s)"))?;
            let now: Bound<'py, PyAny> = imported(py, "datetime", "datetime")?
                .call_method1("now", (imported(py, "datetime", "UTC")?,))?;
            Ok((ttl.call_method1("add_to", (now,))?, elapsed))
        }
        Err(error) => {
            progress_line(
                py,
                &format!("Query diff inspection  ERROR  ({:.2}s)", seconds(started)),
            )?;
            Err(error)
        }
    }
}

/// Refuse existing artifacts, an invalid TTL and unusable query columns; the parsed TTL.
fn inspect_inputs<'py>(
    request: &Bound<'py, PyAny>,
    preparation: &Bound<'py, PyAny>,
    connection: &Bound<'py, PyAny>,
    (left, right): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = request.py();
    let adapter: Bound<'py, PyAny> = native_field(preparation, "adapter")?;
    let location = |artifact: &Bound<'py, PyAny>| -> PyResult<Bound<'py, PyTuple>> {
        PyTuple::new(
            py,
            [
                native_field(artifact, "database")?,
                native_field(artifact, "schema")?,
                native_field(artifact, "name")?,
            ],
        )
    };
    let locations: Bound<'py, PyTuple> = PyTuple::new(py, [location(left)?, location(right)?])?;
    let lookup: Bound<'py, PyAny> = call(
        &host::entry(py, AdapterHost::BuildRelationLookup)?,
        &[
            ("adapter", &adapter),
            ("connection", connection),
            ("locations", &locations.into_any()),
        ],
    )?;
    for artifact in [left, right] {
        let exists: Bound<'py, PyAny> = call(
            &native_field(&lookup, "exists")?,
            &[
                ("database", &native_field(artifact, "database")?),
                ("schema", &native_field(artifact, "schema")?),
                ("name", &native_field(artifact, "name")?),
            ],
        )?;
        if exists.is_truthy()? {
            return Err(failure(
                py,
                format!(
                    "query-diff artifact already exists: {}",
                    native_field(artifact, "relation")?.str()?
                ),
                "C245",
                Some("wait for its TTL cleanup or investigate the reserved-name relation"),
            ));
        }
    }
    let ttl: Bound<'py, PyAny> = record_class(py, RecordId::Duration)?
        .call_method1("parse", (native_field(preparation, "artifact_ttl")?,))?;
    if ttl.is_none() {
        return Err(failure(
            py,
            "diff.query_artifact_ttl must be a positive duration".to_owned(),
            "C228",
            None,
        ));
    }
    let max_columns: Option<i64> = native_field(request, "max_columns")?.extract()?;
    for (sql, label) in [("left_sql", "left_label"), ("right_sql", "right_label")] {
        let columns: Vec<String> = call(
            &native_field(&adapter, "query_column_names")?,
            &[
                ("connection", connection),
                ("sql", &native_field(preparation, sql)?),
            ],
        )?
        .extract()?;
        let label: String = native_field(preparation, label)?.extract()?;
        query_column_problem(&label, &columns, max_columns)
            .map_err(|error| user_failure(py, error))?;
    }
    Ok(ttl)
}
