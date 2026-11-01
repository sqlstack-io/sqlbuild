//! The compile-and-plan pipeline of execution commands: compile, rules gate, then planning.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyModule, PyModuleMethods, PyResult, Python};
use pyo3::types::PyDict;
use pyo3::{IntoPyObject, PyErr, pyfunction, wrap_pyfunction};
use sqlbuild_host::entries::models::ExecutorHost;
use sqlbuild_rules::rule_results::main::default_rules_notice::default_rules_notice;

use crate::bindings::_helpers::boundary::python_calls::{KeywordArguments, keywords};
use crate::bindings::_helpers::compile_driver::phases::monotonic;
use crate::bindings::_helpers::compile_driver::pipeline::{
    compiled_phase, progress, timing_tracker,
};
use crate::bindings::_helpers::compile_driver::pipeline_entries::project_graph;
use crate::bindings::_helpers::compile_driver::plan_result::{
    PlanningInputs, connection_with_hooks, pipeline_result,
};
use crate::bindings::_helpers::compile_driver::selection_phases::{
    apply_unit_test_selectors, resolve_configured_rule_selection,
};
use crate::bindings::_helpers::frontier::stage_capture::write_plan_capture;
use crate::bindings::_helpers::native_host::host;
use crate::bindings::_helpers::records::construct::new_record_with;
use crate::bindings::_helpers::records::construct::{error_class, new_annotated_error};
use crate::bindings::_helpers::records::fields::field as native_field;
use crate::bindings::_helpers::records::fields::replaced;
use crate::bindings::_helpers::records::ids::{ErrorId, RecordId};
use crate::bindings::_helpers::rules::constants::DEFAULT_RULES_NOTICE_ATTR;
use crate::bindings::_helpers::rules::early_sql_lint::EarlySqlLint;
use crate::bindings::_helpers::rules::results::{format_rules_result, rules_progress};
use crate::bindings::_helpers::rules::rule_dispatch::evaluate_rules;
use crate::bindings::_helpers::rules::rules_config::load_rules_config_with_cache;

const GENERIC_DIALECT: &str = "generic";

/// Run compile inputs, assembly, planning, and manifest generation.
#[pyfunction]
#[pyo3(signature = (**options))]
pub(crate) fn run_compile_pipeline<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    let arguments: KeywordArguments<'py> = KeywordArguments::read(
        py,
        "run_compile_pipeline",
        options,
        (&["discovered_inputs", "adapter"], &["options", "hooks"]),
    )?;
    let discovered: Bound<'py, PyAny> = arguments.value("discovered_inputs")?;
    let adapter: Bound<'py, PyAny> = arguments.value("adapter")?;
    let options: Bound<'py, PyAny> =
        apply_unit_test_selectors(&arguments.value("options")?, &discovered)?;
    let hooks: Bound<'py, PyAny> = arguments.value("hooks")?;
    let hooks: Bound<'py, PyAny> = if hooks.is_none() {
        host::entry(py, ExecutorHost::ConnectionHooks)?.call0()?
    } else {
        hooks
    };
    let on_progress: Bound<'py, PyAny> = native_field(&hooks, "on_progress")?;
    let expansion_reuse: Bound<'py, EarlySqlLint> =
        Bound::new(py, EarlySqlLint::new(false, false))?;
    let on_inputs_ready: Bound<'py, PyAny> =
        EarlySqlLint::inputs_ready_hook(&expansion_reuse, GENERIC_DIALECT, &adapter);
    let compiled: Bound<'py, PyAny> = compiled_phase(
        (&discovered, &adapter),
        &options,
        &on_progress,
        Some(&on_inputs_ready),
    )?;
    let project: Bound<'py, PyAny> = native_field(&compiled, "project")?;
    let graph: Bound<'py, PyAny> = project_graph(&project)?;
    run_rules_gate(
        (&discovered, &adapter, expansion_reuse.as_any()),
        (&options, &on_progress),
        (&project, &graph),
    )?;
    planned((&discovered, &adapter), (&options, &hooks), &compiled)
}

/// Run the configured rules and stop the command when any of them finds a fault.
fn run_rules_gate<'py>(
    (discovered, adapter, expansion_reuse): (
        &Bound<'py, PyAny>,
        &Bound<'py, PyAny>,
        &Bound<'py, PyAny>,
    ),
    (options, on_progress): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
    (project, graph): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
) -> PyResult<()> {
    let py: Python<'py> = discovered.py();
    let rules_error: Bound<'py, PyAny> = error_class(py, ErrorId::Rules)?;
    let project_dir: Bound<'py, PyAny> = native_field(discovered, "project_dir")?;
    if project_dir.is_none() {
        return Err(PyErr::from_value(
            rules_error.call1(("compiler rules require a resolved project directory",))?,
        ));
    }
    let config: Bound<'py, PyAny> = load_rules_config_with_cache(
        py,
        project_dir.extract()?,
        !native_field(options, "no_cache")?.is_truthy()?,
    )?
    .into_bound(py);
    progress(on_progress, "Evaluating configured rules...")?;
    let started: f64 = monotonic(py)?;
    let dialect: Bound<'py, PyAny> = adapter.call_method0("sql_analysis_dialect")?;
    let dialect: Bound<'py, PyAny> = if dialect.is_truthy()? {
        dialect
    } else {
        GENERIC_DIALECT.into_pyobject(py)?.into_any()
    };
    let selected_keys: Bound<'py, PyAny> =
        resolve_configured_rule_selection(options, discovered, project)?;
    let result: Bound<'py, PyAny> = evaluate_rules(
        py,
        Some(keywords(
            py,
            &[
                ("graph", graph.clone()),
                ("discovered_inputs", discovered.clone()),
                ("config", config.clone()),
                ("project_dir", project_dir),
                ("dialect", dialect),
                ("selected_keys", selected_keys),
                ("early_lint", expansion_reuse.clone()),
                (
                    "no_sql_analysis",
                    native_field(options, "no_sql_validation")?,
                ),
            ],
        )?),
    )?;
    if !on_progress.is_none() {
        let elapsed: f64 = monotonic(py)? - started;
        let message: String = rules_progress(elapsed, &result, true)?;
        on_progress.call1((message,))?;
    }
    let findings: Bound<'py, PyAny> = native_field(&result, "findings")?;
    if !findings.is_truthy()? {
        return Ok(());
    }
    let summary: Bound<'py, PyAny> = new_record_with(
        py,
        RecordId::RulesResult,
        &[
            ("findings", findings),
            (
                "evaluated_models",
                native_field(&result, "evaluated_models")?,
            ),
            (
                "unevaluated_resources",
                native_field(&result, "unevaluated_resources")?,
            ),
            ("cache_hits", native_field(&result, "cache_hits")?),
            ("cache_misses", native_field(&result, "cache_misses")?),
        ],
    )?;
    let rendered: String = format_rules_result(&summary, false)?;
    let codes: Vec<String> = native_field(&summary, "findings")?
        .try_iter()?
        .map(|finding| native_field(&finding?, "code")?.extract())
        .collect::<PyResult<_>>()?;
    let select_omitted: bool = native_field(&config, "select")?.is_none();
    let notice: Vec<(&str, String)> =
        default_rules_notice(select_omitted, codes.iter().map(String::as_str))
            .map(|notice| (DEFAULT_RULES_NOTICE_ATTR, notice))
            .into_iter()
            .collect();
    Err(new_annotated_error(
        py,
        ErrorId::Rules,
        format!("configured compiler rules failed:\n{rendered}"),
        &notice,
    ))
}

/// Open the planning connection, plan, and record the compile and planning times.
fn planned<'py>(
    (discovered, adapter): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
    (options, hooks): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
    compiled: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = discovered.py();
    let tracker: Bound<'py, PyAny> = timing_tracker(py)?;
    let started: f64 = monotonic(py)?;
    let mut connection: Option<Bound<'py, PyAny>> = None;
    let result: PyResult<Bound<'py, PyAny>> = (|| {
        let opened: Bound<'py, PyAny> = connection_with_hooks(
            adapter,
            &native_field(compiled, "connection_config")?,
            hooks,
        )?;
        connection = Some(opened.clone());
        pipeline_result(
            &PlanningInputs {
                discovered_inputs: discovered,
                adapter,
                connection: &opened,
            },
            &native_field(compiled, "project")?,
            options,
            &native_field(hooks, "on_progress")?,
        )
    })();
    if let Some(connection) = connection.filter(|connection| !connection.is_none()) {
        adapter.call_method1("close", (connection,))?;
    }
    let planning_seconds: f64 = monotonic(py)? - started;
    if !tracker.is_none() {
        tracker.setattr("planning_seconds", planning_seconds)?;
    }
    let result: Bound<'py, PyAny> = result?;
    write_plan_capture(&native_field(&result, "plan_output")?)?;
    replaced(
        &result,
        &[
            (
                "compile_seconds",
                native_field(compiled, "compile_seconds")?,
            ),
            (
                "planning_seconds",
                planning_seconds.into_pyobject(py)?.into_any(),
            ),
        ],
    )
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(run_compile_pipeline, module)?)?;
    Ok(())
}
