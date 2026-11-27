//! `sqb diff`: validate, discover, run a model or raw-query comparison and publish its output.

use pyo3::IntoPyObjectExt;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyErr, PyResult, Python, pyfunction};
use pyo3::types::{PyBool, PyDict, PyDictMethods, PyInt, PyTuple};
use sqlbuild_cli::diff_report::errors::DiffUserError;
use sqlbuild_cli::diff_report::main::diff_exit_code::diff_exit_code;
use sqlbuild_cli::diff_report::main::effective_max_examples::effective_max_examples;
use sqlbuild_cli::diff_report::main::is_incomplete_query_failure::is_incomplete_query_failure;
use sqlbuild_cli::diff_report::main::is_query_diff_request::is_query_diff_request;
use sqlbuild_cli::diff_report::main::size_guard_report::size_guard_report;
use sqlbuild_cli::diff_report::main::validate_diff_request::validate_diff_request;
use sqlbuild_cli::diff_report::models::{
    DiffModelFacts, DiffRequestFacts, FullDiffModelFacts, FullDiffSideFacts, SizeGuardReport,
};
use sqlbuild_cli::diff_report::types::QueryDiffPhase;
use sqlbuild_core::json::main::dumps::dumps;
use sqlbuild_core::json::models::{JsonDialect, StdlibJsonOptions};
use sqlbuild_host::entries::models::{AdapterHost, ExecutorHost};

use crate::bindings::_helpers::command_runtime::connection::resolve_target_connection_config;
use crate::bindings::_helpers::dbt_interop::reference_resolver::resolve_external_sql_reference_resolver;
use crate::bindings::_helpers::diff_report::facts::diff_facts;
use crate::bindings::_helpers::diff_report::output::publish_diff_output;
use crate::bindings::_helpers::diff_report::query::{
    execute_query_diff, execution_options, prepare_query_diff, project_adapter,
};
use crate::bindings::_helpers::diff_report::support::{
    call, caused, cli_error, error_text, is_exception, matches_class, matches_native, none, object,
    request_facts, user_error_parts,
};
use sqlbuild_python_base::boundary::main::imported::imported;
use sqlbuild_python_base::boundary::main::keywords::keywords;
use sqlbuild_python_base::frontier::main::emit_error::emit_error;
use sqlbuild_python_base::native_host::main::connection_failure::connection_failure;
use sqlbuild_python_base::native_host::main::host_entry::host_entry;
use sqlbuild_python_base::records::main::construct::error_class::error_class;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::ErrorId;
use sqlbuild_python_compile::compile_driver::main::stages::discover_project_inputs::discover_project_inputs;
use sqlbuild_python_compile::compile_driver::main::stages::run_diff_pipeline::run_diff_pipeline;
use sqlbuild_python_front::contracts::main::resolve_target_config::resolve_target_config;
use sqlbuild_python_front::contracts::main::resolve_target_name::resolve_target_name;
use sqlbuild_python_front::discovery::main::project::discover_project_configuration::discover_project_configuration;
use sqlbuild_python_project::planner_selection::main::reject_unit_test_selectors::reject_unit_test_selectors;

const DEFAULT_DIFF_MAX_FULL_ROWS: i64 = 10_000_000;
const ADAPTER_ERRORS: AdapterHost = AdapterHost::AdapterUserError;
const EXECUTOR_INPUT_ERRORS: ExecutorHost = ExecutorHost::ExecutorInputError;
const SIZE_GUARD_ERRORS: ExecutorHost = ExecutorHost::FullDiffSizeGuardError;
const OUTCOME_ERRORS: ErrorId = ErrorId::QueryDiffOutcome;
const USER_ERRORS: ErrorId = ErrorId::CliUser;
const ROW_DIFF_KEY_ERROR_PREFIX: &str = "row diff ";
const ROW_DIFF_KEY_ERROR_FRAGMENT: &str = " unique_key values";

fn user(message: &str, code: &str, help: Option<&str>) -> DiffUserError {
    DiffUserError {
        message: message.to_owned(),
        code: code.to_owned(),
        help: help.map(str::to_owned),
    }
}

/// `CliUserError` from a pure rejection.
fn rejected(py: Python<'_>, error: &DiffUserError) -> PyErr {
    cli_error(py, ErrorId::CliUser, error)
}

/// The project directory and discovered inputs, after the request's flags validate.
fn resolve_invocation<'py>(
    request: &Bound<'py, PyAny>,
    facts: &DiffRequestFacts,
) -> PyResult<(Bound<'py, PyAny>, Bound<'py, PyAny>)> {
    let py: Python<'py> = request.py();
    let query_mode: bool = is_query_diff_request(facts);
    if let Err(error) = validate_diff_request(facts) {
        return Err(cli_error(
            py,
            if query_mode {
                ErrorId::QueryDiffIncomplete
            } else {
                ErrorId::CliUser
            },
            &error,
        ));
    }
    let project_dir: Bound<'py, PyAny> = match native_field(request, "project_dir")? {
        directory if !directory.is_none() => directory,
        _ => imported(py, "pathlib", "Path")?.call_method0("cwd")?,
    };
    let discovered: Bound<'py, PyAny> = if query_mode {
        discover_project_configuration(py, &project_dir)?
    } else {
        discover_project_inputs(
            py,
            Some(keywords(py, &[("project_dir", project_dir.clone())])?),
        )?
    };
    reject_unit_test_selectors(
        &native_field(request, "select")?,
        &native_field(request, "exclude")?,
        &discovered.clone(),
    )?;
    if facts.select.is_empty() && !query_mode {
        return Err(rejected(
            py,
            &user("diff requires --select in v1", "C204", None),
        ));
    }
    Ok((project_dir, discovered))
}

/// `message`, `code` and `help` re-raised as an incomplete or failed query-diff outcome.
fn query_outcome_error(
    py: Python<'_>,
    incomplete: bool,
    parts: &DiffUserError,
    cause: PyErr,
) -> PyErr {
    let class: ErrorId = if incomplete {
        ErrorId::QueryDiffIncomplete
    } else {
        ErrorId::QueryDiffExecution
    };
    caused(py, cli_error(py, class, parts), cause)
}

fn execution_failure(py: Python<'_>, code: &str, cause: PyErr) -> PyResult<PyErr> {
    let parts: DiffUserError = user(&error_text(py, &cause)?, code, None);
    Ok(query_outcome_error(py, false, &parts, cause))
}

/// Map a raw-query execution failure to its incomplete or execution-failure outcome.
fn query_execution_error(py: Python<'_>, error: PyErr) -> PyResult<PyErr> {
    if matches_native(py, &error, OUTCOME_ERRORS)? || !is_exception(py, &error) {
        return Ok(error);
    }
    let value: Bound<'_, PyAny> = error.value(py).clone().into_any();
    if matches_class(py, &error, EXECUTOR_INPUT_ERRORS)? {
        let code: String = match value.getattr_opt("code")? {
            Some(code) => code.str()?.extract()?,
            None => "C254".to_owned(),
        };
        let help: Option<String> = match value.getattr_opt("help")? {
            Some(help) if !help.is_none() => Some(help.extract()?),
            _ => None,
        };
        let parts: DiffUserError = DiffUserError {
            message: error_text(py, &error)?,
            code,
            help,
        };
        return Ok(query_outcome_error(py, true, &parts, error));
    }
    if matches_native(py, &error, USER_ERRORS)? {
        let parts: DiffUserError = user_error_parts(&value)?;
        let incomplete: bool = is_incomplete_query_failure(QueryDiffPhase::Execution, &parts.code);
        return Ok(query_outcome_error(py, incomplete, &parts, error));
    }
    if matches_class(py, &error, ADAPTER_ERRORS)? {
        let parts: DiffUserError = user_error_parts(&value)?;
        let incomplete: bool = parts.message.starts_with(ROW_DIFF_KEY_ERROR_PREFIX)
            && parts.message.contains(ROW_DIFF_KEY_ERROR_FRAGMENT);
        return Ok(query_outcome_error(py, incomplete, &parts, error));
    }
    execution_failure(py, "C255", error)
}

fn run_query_diff(
    request: &Bound<'_, PyAny>,
    invocation: (&Bound<'_, PyAny>, &Bound<'_, PyAny>),
) -> PyResult<i32> {
    let py: Python<'_> = request.py();
    let preparation: Bound<'_, PyAny> = match prepare_query_diff(request, invocation) {
        Ok(preparation) => preparation,
        Err(error) if matches_native(py, &error, USER_ERRORS)? => {
            let parts: DiffUserError = user_error_parts(&error.value(py).clone().into_any())?;
            let incomplete: bool =
                is_incomplete_query_failure(QueryDiffPhase::Preparation, &parts.code);
            return Err(query_outcome_error(py, incomplete, &parts, error));
        }
        Err(error) if is_exception(py, &error) => {
            return Err(execution_failure(py, "C253", error)?);
        }
        Err(error) => return Err(error),
    };
    let (result, phases) = match execute_query_diff(request, &preparation) {
        Ok(outcome) => outcome,
        Err(error) => return Err(query_execution_error(py, error)?),
    };
    let models: Vec<DiffModelFacts> = diff_facts(&result)?;
    let schema_only: bool = native_field(request, "schema_only")?.is_truthy()?;
    let (outcome, exit_code) = diff_exit_code(&models, !schema_only);
    let published: PyResult<()> = publish_diff_output(
        request,
        (
            &native_field(&preparation, "left_label")?.extract::<String>()?,
            &native_field(&preparation, "right_label")?.extract::<String>()?,
        ),
        (
            native_field(&preparation, "effective_max_column_examples")?.extract()?,
            native_field(&preparation, "effective_max_row_only_examples")?.extract()?,
        ),
        (&models, Some(phases), Some(outcome)),
    );
    match published {
        Ok(()) => Ok(exit_code),
        Err(error) if is_exception(py, &error) => {
            let parts: DiffUserError = user(
                &format!(
                    "failed to publish query diff output: {}",
                    error_text(py, &error)?
                ),
                "C257",
                None,
            );
            Err(query_outcome_error(py, false, &parts, error))
        }
        Err(error) => Err(error),
    }
}

pub(crate) fn target_max_full_rows<'py>(
    discovered: &Bound<'py, PyAny>,
    target: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = discovered.py();
    let configured: Bound<'py, PyAny> = resolve_target_config(
        py,
        &native_field(discovered, "project_config")?,
        &native_field(discovered, "local_config")?,
        target,
    )?
    .getattr("diff")?
    .getattr("max_full_rows")?;
    if configured.is_none() {
        return DEFAULT_DIFF_MAX_FULL_ROWS.into_bound_py_any(py);
    }
    if configured.is_instance_of::<PyInt>() && !configured.is_instance_of::<PyBool>() {
        return Ok(configured);
    }
    Ok(none(py))
}

/// The active target when only FROM was given, rejecting a missing or identical target.
fn diff_to_target<'py>(
    request: &Bound<'py, PyAny>,
    discovered: &Bound<'py, PyAny>,
    from_target: &str,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = request.py();
    let to_name: Bound<'py, PyAny> = native_field(request, "to_name")?;
    if !to_name.is_none() {
        return Ok(to_name);
    }
    let active: Bound<'py, PyAny> = resolve_target_name(
        py,
        &native_field(discovered, "project_config")?,
        &native_field(discovered, "local_config")?,
        &none(py),
    )
    .and_then(|value| pyo3::IntoPyObjectExt::into_bound_py_any(value, py))?;
    if active.is_none() {
        return Err(rejected(
            py,
            &user(
                "model diff requires FROM:TO when the project has no active target",
                "C271",
                Some("set default_target, or pass both targets as FROM:TO"),
            ),
        ));
    }
    if active.extract::<String>()? == from_target {
        return Err(rejected(
            py,
            &user(
                &format!(
                    "diff FROM '{from_target}' is the active target, so there is nothing to compare"
                ),
                "C272",
                Some("pass both targets as FROM:TO"),
            ),
        ));
    }
    Ok(active)
}

fn side_facts(side: &Bound<'_, PyAny>) -> PyResult<FullDiffSideFacts> {
    let detail: Bound<'_, PyAny> = native_field(side, "detail")?;
    Ok(FullDiffSideFacts {
        target: native_field(side, "target")?.extract()?,
        relation: native_field(side, "relation")?.extract()?,
        row_count: native_field(side, "row_count")?.extract()?,
        max_rows: native_field(side, "max_rows")?.extract()?,
        exceeds_limit: native_field(side, "exceeds_limit")?.is_truthy()?,
        detail: if detail.is_none() {
            None
        } else {
            Some(detail.str()?.extract()?)
        },
    })
}

/// `DiffSizeGuardError` (C270) for the guard's blocked models.
fn size_guard_error(
    py: Python<'_>,
    facts: &DiffRequestFacts,
    targets: (&str, &str),
    error: PyErr,
) -> PyResult<PyErr> {
    let value: Bound<'_, PyAny> = error.value(py).clone().into_any();
    let mut blocked: Vec<FullDiffModelFacts> = Vec::new();
    for model in native_field(&value, "blocked")?.try_iter()? {
        let model: Bound<'_, PyAny> = model?;
        blocked.push(FullDiffModelFacts {
            name: native_field(&model, "name")?.extract()?,
            has_cursor: native_field(&model, "has_cursor")?.is_truthy()?,
            left: side_facts(&native_field(&model, "left")?)?,
            right: side_facts(&native_field(&model, "right")?)?,
        });
    }
    let message: String = native_field(&value, "message")?.extract()?;
    let report: SizeGuardReport = size_guard_report(facts, targets, &message, &blocked);
    let details_text: String = dumps(
        &report.details,
        &JsonDialect::Stdlib(StdlibJsonOptions::new(None)),
    )
    .map_err(emit_error)?;
    let details: Bound<'_, PyAny> = imported(py, "json", "loads")?.call1((details_text,))?;
    let keywords: Bound<'_, PyDict> = PyDict::new(py);
    keywords.set_item("details", details)?;
    keywords.set_item("code", "C270")?;
    keywords.set_item("help", report.help)?;
    let replacement: Bound<'_, PyAny> = error_class(py, ErrorId::DiffSizeGuard)?
        .call((report.message.as_str(),), Some(&keywords))?;
    Ok(caused(py, PyErr::from_value(replacement), error))
}

fn run_model_diff(
    request: &Bound<'_, PyAny>,
    facts: &DiffRequestFacts,
    (project_dir, discovered): (&Bound<'_, PyAny>, &Bound<'_, PyAny>),
) -> PyResult<i32> {
    let py: Python<'_> = request.py();
    let Some(from_name) = facts.from_name.as_deref() else {
        return Err(rejected(
            py,
            &user("model diff requires FROM or FROM:TO", "C224", None),
        ));
    };
    let to_target: Bound<'_, PyAny> = diff_to_target(request, discovered, from_name)?;
    let to_name: String = to_target.extract()?;
    let targets: Bound<'_, PyAny> =
        native_field(discovered, "project_config")?.getattr("targets")?;
    if !targets.contains(from_name)? {
        return Err(rejected(
            py,
            &user(
                &format!("unknown diff FROM target '{from_name}'"),
                "C205",
                None,
            ),
        ));
    }
    if !targets.contains(&to_name)? {
        return Err(rejected(
            py,
            &user(&format!("unknown diff TO target '{to_name}'"), "C206", None),
        ));
    }
    let from_target: Bound<'_, PyAny> = object(py, from_name)?;
    let (_, adapter) = project_adapter(discovered, project_dir)?;
    let cli_vars: Bound<'_, PyAny> = native_field(request, "cli_vars")?;
    let connection_config: Bound<'_, PyAny> = resolve_target_connection_config(
        py,
        Some(keywords(
            py,
            &[
                ("discovered_inputs", discovered.clone()),
                ("project_dir", project_dir.clone()),
                ("target_name", to_target.clone()),
                ("cli_vars", cli_vars.clone()),
            ],
        )?),
    )?;
    let resolver: Bound<'_, PyAny> =
        resolve_external_sql_reference_resolver(project_dir.clone(), discovered.clone())?;
    let pipeline: Bound<'_, PyTuple> = run_diff_pipeline(
        py,
        Some(keywords(
            py,
            &[
                ("discovered_inputs", discovered.clone()),
                ("adapter", adapter.clone()),
                ("from_target", from_target.clone()),
                ("to_target", to_target.clone()),
                ("resolved_connection", connection_config.clone()),
                (
                    "no_sql_validation",
                    native_field(request, "no_sql_validation")?,
                ),
                ("select", native_field(request, "select")?),
                ("exclude", native_field(request, "exclude")?),
                ("cli_vars", cli_vars.clone()),
                ("external_sql_reference_resolver", resolver.clone()),
            ],
        )?),
    )
    .and_then(|value| pyo3::IntoPyObjectExt::into_bound_py_any(value, py))?
    .cast_into()?;
    let selected: Bound<'_, PyAny> = pipeline.get_item(2)?;
    if !selected.is_truthy()? {
        return Err(rejected(
            py,
            &user(
                "no diffable models found in the selected scope",
                "C207",
                None,
            ),
        ));
    }
    let verbose: bool = native_field(request, "verbose")?.is_truthy()?;
    let column_examples: i64 = effective_max_examples(facts.max_column_examples, verbose);
    let row_only_examples: i64 = effective_max_examples(facts.max_row_only_examples, verbose);
    let limits: Bound<'_, PyAny> = if facts.full || facts.schema_only || facts.bounded.is_some() {
        none(py)
    } else {
        call(
            &host_entry(py, ExecutorHost::FullDiffSizeLimits)?,
            &[
                ("left_target", &from_target),
                ("right_target", &to_target),
                (
                    "left_max_rows",
                    &target_max_full_rows(discovered, &from_target)?,
                ),
                (
                    "right_max_rows",
                    &target_max_full_rows(discovered, &to_target)?,
                ),
            ],
        )?
    };
    let connection: Bound<'_, PyAny> = adapter
        .call_method1("connect", (&connection_config,))
        .map_err(|error| connection_failure(&adapter, &error))?;
    let extras: [(&str, Bound<'_, PyAny>); 5] = [
        ("bounded", native_field(request, "bounded")?),
        ("max_column_examples", object(py, column_examples)?),
        ("max_row_only_examples", object(py, row_only_examples)?),
        ("max_models", native_field(request, "max_models")?),
        ("full_size_limits", limits),
    ];
    let extra: Vec<(&str, &Bound<'_, PyAny>)> =
        extras.iter().map(|(key, value)| (*key, value)).collect();
    let executed: PyResult<Bound<'_, PyAny>> =
        execution_options(request, &extra).and_then(|options| {
            call(
                &host_entry(py, ExecutorHost::ExecuteDiff)?,
                &[
                    ("adapter", &adapter),
                    ("connection", &connection),
                    ("left_project", &pipeline.get_item(0)?),
                    ("right_project", &pipeline.get_item(1)?),
                    ("selected_names", &selected),
                    ("options", &options),
                ],
            )
        });
    let executed: PyResult<Bound<'_, PyAny>> = match executed {
        Err(error) if matches_class(py, &error, SIZE_GUARD_ERRORS)? => {
            Err(size_guard_error(py, facts, (from_name, &to_name), error)?)
        }
        other => other,
    };
    adapter.call_method1("close", (&connection,))?;
    let result: Bound<'_, PyAny> = executed?;
    let models: Vec<DiffModelFacts> = diff_facts(&result)?;
    publish_diff_output(
        request,
        (from_name, &to_name),
        (column_examples, row_only_examples),
        (&models, None, None),
    )?;
    Ok(diff_exit_code(&models, false).1)
}

/// Run one `sqb diff` request; the exit code.
#[pyfunction]
#[pyo3(signature = (*, request))]
pub(crate) fn run_diff_command(py: Python<'_>, request: &Bound<'_, PyAny>) -> PyResult<i32> {
    let facts: DiffRequestFacts = request_facts(request)?;
    let query_mode: bool = is_query_diff_request(&facts);
    let (project_dir, discovered) = match resolve_invocation(request, &facts) {
        Ok(invocation) => invocation,
        Err(error)
            if query_mode
                && !matches_native(py, &error, OUTCOME_ERRORS)?
                && is_exception(py, &error) =>
        {
            return Err(execution_failure(py, "C252", error)?);
        }
        Err(error) => return Err(error),
    };
    if query_mode {
        return run_query_diff(request, (&project_dir, &discovered));
    }
    run_model_diff(request, &facts, (&project_dir, &discovered))
}

/// Materialize, compare and clean one prepared raw-query pair; the diff result.
#[pyfunction]
#[pyo3(name = "execute_query_diff", signature = (*, request, preparation))]
pub(crate) fn execute_prepared_query_diff<'py>(
    request: &Bound<'py, PyAny>,
    preparation: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    execute_query_diff(request, preparation).map(|(result, _)| result)
}
