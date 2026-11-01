//! Plan selected SQLBuild work against the live adapter and attach it to the interop plan.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyModule, PyModuleMethods, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods};
use pyo3::{pyfunction, wrap_pyfunction};
use sqlbuild_host::entries::models::ExecutorHost;

use crate::bindings::_helpers::boundary::python_calls::{KeywordArguments, call_with, keywords};
use crate::bindings::_helpers::build_command::support::{boolean, keyword_relay, none};
use crate::bindings::_helpers::command_runtime::connection::{
    anchor_local_database_path, with_dbt_profile,
};
use crate::bindings::_helpers::command_runtime::warehouses::apply_command_warehouse;
use crate::bindings::_helpers::compile_attachment::effective_config::build_effective_connection_config;
use crate::bindings::_helpers::compile_driver::deferral::resolve_model_deferral_inputs;
use crate::bindings::_helpers::compile_driver::plan_result::connection_with_hooks;
use crate::bindings::_helpers::native_host::host;
use crate::bindings::_helpers::planner_entries::display_plan::build_display_only_sqlbuild_plan;
use crate::bindings::_helpers::planner_entries::execution_plan::build_execution_plan;
use crate::bindings::_helpers::records::construct::error_class;
use crate::bindings::_helpers::records::construct::{new_record, new_record_with};
use crate::bindings::_helpers::records::fields::field as native_field;
use crate::bindings::_helpers::records::fields::replaced;
use crate::bindings::_helpers::records::ids::{ErrorId, RecordId};

const BUILD_OPTIONS: (&[&str], &[&str]) = (&["environment", "request", "hooks"], &[]);
const ATTACH_OPTIONS: (&[&str], &[&str]) = (
    &[
        "plan",
        "project_dir",
        "discovered_inputs",
        "compiled",
        "sqlbuild_args",
        "connection_progress",
        "on_progress",
    ],
    &[],
);

pub(crate) fn connection_config<'py>(
    environment: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py = environment.py();
    let discovered = native_field(environment, "discovered_inputs")?;
    let raw = build_effective_connection_config(&discovered.clone(), None, None, false)?;
    let raw: Bound<'_, PyDict> = raw.cast_into()?;
    let project_dir = native_field(environment, "project_dir")?;
    let adapter_name = native_field(environment, "adapter_name")?;
    let config: Bound<'_, PyDict> = with_dbt_profile(
        &raw,
        raw.copy()?,
        (&project_dir, &adapter_name),
        (&discovered, &py.None().into_bound(py)),
    )?;
    anchor_local_database_path(
        &config,
        (&project_dir, &adapter_name.str()?.extract::<String>()?),
    )?;
    let config = config.into_any();
    apply_command_warehouse(
        py,
        Some(keywords(
            py,
            &[
                ("config", config),
                ("adapter_name", native_field(environment, "adapter_name")?),
                ("discovered_inputs", discovered),
                ("project_dir", native_field(environment, "project_dir")?),
                ("selected_target", none(py)),
            ],
        )?),
    )
}

fn flag_value<'py>(args: &Bound<'py, PyAny>, flag: &str) -> PyResult<Bound<'py, PyAny>> {
    if !args.contains(flag)? {
        return Ok(none(args.py()));
    }
    let index: usize = args.call_method1("index", (flag,))?.extract()?;
    if index + 1 < args.len()? {
        args.get_item(index + 1)
    } else {
        Ok(none(args.py()))
    }
}

pub(crate) fn connection_hooks<'py>(
    reporter: &Bound<'py, PyAny>,
    callback: Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py = reporter.py();
    let mut pairs = vec![("on_progress", callback)];
    if !reporter.is_none() {
        pairs.push((
            "on_connection_start",
            native_field(reporter, "on_connection_start")?,
        ));
        for name in ["on_connection_complete", "on_connection_error"] {
            pairs.push((
                name,
                keyword_relay(
                    native_field(reporter, name)?,
                    &["connection_count", "elapsed_seconds"],
                )?,
            ));
        }
    }
    call_with(&host::entry(py, ExecutorHost::ConnectionHooks)?, &pairs)
}

/// Open through the shared adapter boundary while retaining keyword-only timing callbacks.
fn open_connection<'py>(
    environment: &Bound<'py, PyAny>,
    config: Bound<'py, PyAny>,
    hooks: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py = environment.py();
    let mut pairs = vec![(
        "on_connection_start",
        native_field(hooks, "on_connection_start")?,
    )];
    for name in ["on_connection_complete", "on_connection_error"] {
        let callback = native_field(hooks, name)?;
        pairs.push((
            name,
            if callback.is_none() {
                callback
            } else {
                keyword_relay(callback, &["connection_count", "elapsed_seconds"])?
            },
        ));
    }
    let relayed = call_with(&host::entry(py, ExecutorHost::ConnectionHooks)?, &pairs)?;
    connection_with_hooks(&native_field(environment, "adapter")?, &config, &relayed)
}

/// Build the SQLBuild plan from current warehouse facts, closing the custom adapter connection.
#[pyfunction]
#[pyo3(signature = (**options))]
pub(crate) fn build_sqlbuild_plan_output<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    let arguments =
        KeywordArguments::read(py, "build_sqlbuild_plan_output", options, BUILD_OPTIONS)?;
    let request = arguments.value("request")?;
    let selected = native_field(&request, "selected_model_names")?;
    if !selected.is_truthy()? {
        return Ok(none(py));
    }
    let environment = arguments.value("environment")?;
    let hooks = arguments.value("hooks")?;
    let connection = open_connection(&environment, connection_config(&environment)?, &hooks)?;
    let planned = (|| {
        let args = native_field(&request, "sqlbuild_args")?;
        let deferral = resolve_model_deferral_inputs(
            py,
            Some(keywords(
                py,
                &[
                    ("project", native_field(&environment, "project")?),
                    (
                        "discovered_inputs",
                        native_field(&environment, "discovered_inputs")?,
                    ),
                    ("adapter", native_field(&environment, "adapter")?),
                    ("connection", connection.clone()),
                    ("defer_to", flag_value(&args, "--defer-to")?),
                ],
            )?),
        )?;
        let cursor = new_record_with(
            py,
            RecordId::CursorOverrides,
            &[
                ("start_ts", flag_value(&args, "--start-cursor-ts")?),
                ("end_ts", flag_value(&args, "--end-cursor-ts")?),
                ("start_int", flag_value(&args, "--start-cursor-int")?),
                ("end_int", flag_value(&args, "--end-cursor-int")?),
            ],
        )?;
        let full_refresh = boolean(py, args.contains("--full-refresh")?);
        let selection = new_record_with(
            py,
            RecordId::PlannerSelection,
            &[("select", selected.clone())],
        )?;
        let overrides = new_record_with(
            py,
            RecordId::PlannerOverrides,
            &[
                ("cursor_overrides", cursor),
                ("full_refresh", full_refresh.clone()),
                (
                    "external_blocked_model_names",
                    native_field(&request, "external_blocked_model_names")?,
                ),
            ],
        )?;
        let discovered = native_field(&environment, "discovered_inputs")?;
        let result = build_execution_plan(
            py,
            Some(keywords(
                py,
                &[
                    ("project", native_field(&environment, "project")?),
                    ("adapter", native_field(&environment, "adapter")?),
                    ("connection", connection.clone()),
                    ("selection", selection),
                    ("overrides", overrides),
                    ("deferral", deferral),
                    (
                        "policies",
                        new_record(py, RecordId::PlannerPolicies, (), None)?,
                    ),
                    ("on_progress", native_field(&hooks, "on_progress")?),
                    (
                        "project_config",
                        native_field(&discovered, "project_config")?,
                    ),
                    ("local_config", native_field(&discovered, "local_config")?),
                ],
            )?),
        );
        match result {
            Err(error) if error.is_instance(py, &error_class(py, ErrorId::PlannerInput)?) => {
                build_display_only_sqlbuild_plan(
                    &native_field(&environment, "project")?,
                    &selected,
                    full_refresh.extract::<bool>()?,
                )
            }
            result => result,
        }
    })();
    native_field(&environment, "adapter")?.call_method1("close", (connection,))?;
    planned
}

/// Attach live SQLBuild planning when the selection contains SQLBuild models.
#[pyfunction]
#[pyo3(signature = (**options))]
pub(crate) fn attach_sqlbuild_plan_output<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    let args = KeywordArguments::read(py, "attach_sqlbuild_plan_output", options, ATTACH_OPTIONS)?;
    let compiled = args.value("compiled")?;
    let plan = args.value("plan")?;
    let environment = new_record_with(
        py,
        RecordId::DbtPlanEnvironment,
        &[
            ("project_dir", args.value("project_dir")?),
            ("discovered_inputs", args.value("discovered_inputs")?),
            ("project", native_field(&compiled, "project")?),
            ("adapter", native_field(&compiled, "adapter")?),
            ("adapter_name", native_field(&compiled, "adapter_name")?),
        ],
    )?;
    let request = new_record_with(
        py,
        RecordId::DbtSqlbuildPlanRequest,
        &[
            (
                "selected_model_names",
                native_field(&plan, "selection")?.getattr("sqlbuild_model_names")?,
            ),
            ("sqlbuild_args", args.value("sqlbuild_args")?),
        ],
    )?;
    let output = build_sqlbuild_plan_output(
        py,
        Some(keywords(
            py,
            &[
                ("environment", environment),
                ("request", request),
                (
                    "hooks",
                    connection_hooks(
                        &args.value("connection_progress")?,
                        args.value("on_progress")?,
                    )?,
                ),
            ],
        )?),
    )?;
    if output.is_none() {
        return Ok(plan);
    }
    replaced(&plan, &[("sqlbuild_plan_output", output)])
}

/// Prefer stderr for dbt subprocess failure details, retaining Python whitespace semantics.
#[pyfunction]
pub(crate) fn dbt_failure_detail<'py>(result: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let stderr = native_field(result, "stderr")?;
    let detail = if stderr.is_truthy()? {
        stderr
    } else {
        native_field(result, "stdout")?
    }
    .call_method0("strip")?;
    if detail.is_truthy()? {
        Ok(detail)
    } else {
        Ok(none(result.py()))
    }
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(build_sqlbuild_plan_output, module)?)?;
    module.add_function(wrap_pyfunction!(attach_sqlbuild_plan_output, module)?)?;
    module.add_function(wrap_pyfunction!(dbt_failure_detail, module)?)
}
