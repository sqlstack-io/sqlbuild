//! The compile-and-plan pipeline of execution commands: compile, rules gate, then planning.

use sqlbuild_python_planner::planner_phases::constants::CONNECTING;
use sqlbuild_python_planner::planner_phases::constants::EVALUATING_RULES;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyModule, PyModuleMethods, PyResult, Python};
use pyo3::types::{PyDict, PyTuple};
use pyo3::{IntoPyObject, PyErr, pyfunction, wrap_pyfunction};
use sqlbuild_host::entries::models::ExecutorHost;
use sqlbuild_python_planner::live_reads::models::LiveReads;
use sqlbuild_python_planner::planner_phases::constants::COMPILING;
use sqlbuild_python_planner::planner_phases::main::timing::timed::timed;
use sqlbuild_python_planner::planner_warehouse::models::PlannerReads;
use sqlbuild_rules::lint_text::main::counts_toward_exit::counts_toward_exit;
use sqlbuild_rules::rule_results::main::default_rules_notice::default_rules_notice;
use sqlbuild_rules::rule_results::main::format_skipped_models::format_skipped_models;

use crate::bindings::_helpers::compile_driver::plan_result::PlanningInputs;
use crate::bindings::_helpers::compile_driver::plan_result::connection_with_hooks;
use crate::bindings::_helpers::compile_driver::plan_result::pipeline_result;
use crate::bindings::_helpers::compile_driver::rules_at_load::rules_ran_at_load;
use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::boundary::main::keywords::keywords;
use sqlbuild_python_base::boundary::models::KeywordArguments;
use sqlbuild_python_base::native_host::main::host_entry::host_entry;
use sqlbuild_python_base::records::main::construct::error_class::error_class;
use sqlbuild_python_base::records::main::construct::new_annotated_error::new_annotated_error;
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::main::fields::replaced::replaced;
use sqlbuild_python_base::records::types::ErrorId;
use sqlbuild_python_base::records::types::RecordId;
use sqlbuild_python_compile::compile_driver::main::setup::compiled_phase::compiled_phase;
use sqlbuild_python_compile::compile_driver::main::setup::progress::progress;
use sqlbuild_python_compile::compile_driver::main::setup::project_graph::project_graph;
use sqlbuild_python_compile::compile_driver::main::setup::project_target_settings::project_target_settings;
use sqlbuild_python_compile::compile_driver::main::setup::python_node_count::python_node_count;
use sqlbuild_python_compile::compile_driver::main::setup::timing_tracker::timing_tracker;
use sqlbuild_python_compile::compile_driver::main::stages::apply_unit_test_selectors::apply_unit_test_selectors;
use sqlbuild_python_compile::compile_driver::main::stages::monotonic::monotonic;
use sqlbuild_python_compile::compile_driver::main::stages::resolve_configured_rule_selection::resolve_configured_rule_selection;
use sqlbuild_python_compile::frontier::main::write_plan_capture::write_plan_capture;
use sqlbuild_python_compile::rules::constants::DEFAULT_RULES_NOTICE_ATTR;
use sqlbuild_python_compile::rules::main::checking::format_rules_result::format_rules_result;
use sqlbuild_python_compile::rules::main::checking::rules_progress::rules_progress;
use sqlbuild_python_compile::rules::main::configuration::load_rules_config_with_cache::load_rules_config_with_cache;
use sqlbuild_python_compile::rules::main::rule::evaluate_rules::evaluate_rules;
use sqlbuild_python_compile::rules::models::EarlySqlLint;

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
    planned_project(
        (
            &arguments.value("discovered_inputs")?,
            &arguments.value("adapter")?,
        ),
        (&arguments.value("options")?, arguments.value("hooks")?),
        (None, None),
    )
}

/// Compile unless `held` is compiled, gate on rules unless they ran at load, and plan.
pub(crate) fn planned_project<'py>(
    (discovered, adapter): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
    (options, hooks): (&Bound<'py, PyAny>, Bound<'py, PyAny>),
    (held, planned_through): (Option<&Bound<'py, PyAny>>, Option<&Bound<'py, PyAny>>),
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = discovered.py();
    let options: Bound<'py, PyAny> = apply_unit_test_selectors(options, discovered)?;
    let hooks: Bound<'py, PyAny> = if hooks.is_none() {
        call_with(&host_entry(py, ExecutorHost::ConnectionHooks)?, &[])?
    } else {
        hooks
    };
    let on_progress: Bound<'py, PyAny> = native_field(&hooks, "on_progress")?;
    let expansion_reuse: Bound<'py, EarlySqlLint> =
        Bound::new(py, EarlySqlLint::new(false, false))?;
    let compiled: Bound<'py, PyAny> = match held {
        Some(held) => {
            progress(
                &on_progress,
                "Planning from the published compiled project.",
            )?;
            let configured: Bound<'py, PyAny> = native_field(&options, "connection_config")?;
            if configured.is_none() {
                held.clone()
            } else {
                replaced(held, &[("connection_config", configured)])?
            }
        }
        None => {
            let on_inputs_ready: Bound<'py, PyAny> =
                EarlySqlLint::inputs_ready_hook(&expansion_reuse, GENERIC_DIALECT, adapter);
            timed(COMPILING, || {
                compiled_phase(
                    (discovered, adapter),
                    &options,
                    &on_progress,
                    Some(&on_inputs_ready),
                )
            })?
        }
    };
    if !rules_ran_at_load() {
        let project: Bound<'py, PyAny> = native_field(&compiled, "project")?;
        timed(EVALUATING_RULES, || {
            let graph: Bound<'py, PyAny> = project_graph(&project)?;
            run_rules_gate(
                (discovered, adapter, expansion_reuse.as_any()),
                (&options, &on_progress),
                (&project, &graph),
            )
        })?;
    }
    planned(
        (discovered, adapter),
        (&options, &hooks),
        (&compiled, planned_through),
    )
}

/// The failing findings' `RulesResult`, their rendered text and the default-rules notice.
pub(crate) type RuleFailures<'py> = (Bound<'py, PyAny>, String, Option<String>);

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
    let Some((_, rendered, notice)) = configured_rule_failures(
        (discovered, adapter, expansion_reuse),
        (options, on_progress),
        (project, graph),
    )?
    else {
        return Ok(());
    };
    let notice: Vec<(&str, String)> = notice
        .map(|notice| (DEFAULT_RULES_NOTICE_ATTR, notice))
        .into_iter()
        .collect();
    Err(new_annotated_error(
        discovered.py(),
        ErrorId::Rules,
        format!("configured compiler rules failed:\n{rendered}"),
        &notice,
    ))
}

/// Evaluate the configured rules; the failing findings, or `None` when none fails.
pub(crate) fn configured_rule_failures<'py>(
    (discovered, adapter, expansion_reuse): (
        &Bound<'py, PyAny>,
        &Bound<'py, PyAny>,
        &Bound<'py, PyAny>,
    ),
    (options, on_progress): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
    (project, graph): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
) -> PyResult<Option<RuleFailures<'py>>> {
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
        let note: Option<String> = format_skipped_models(
            &native_field(&result, "skipped_models")?.extract::<Vec<_>>()?,
            native_field(options, "verbose")?.is_truthy()?,
        );
        for line in std::iter::once(message).chain(note.map(|note| format!("note: {note}"))) {
            on_progress.call1((line,))?;
        }
    }
    let mut failing: Vec<Bound<'py, PyAny>> = Vec::new();
    for finding in native_field(&result, "findings")?.try_iter()? {
        let finding: Bound<'py, PyAny> = finding?;
        if counts_toward_exit(&native_field(&finding, "code")?.extract::<String>()?) {
            failing.push(finding);
        }
    }
    if failing.is_empty() {
        return Ok(None);
    }
    let findings: Bound<'py, PyAny> = PyTuple::new(py, failing)?.into_any();
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
    let notice: Option<String> =
        default_rules_notice(select_omitted, codes.iter().map(String::as_str));
    Ok(Some((summary, rendered, notice)))
}

/// Open the planning connection, plan, and record the compile and planning times.
fn planned<'py>(
    (discovered, adapter): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
    (options, hooks): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
    (compiled, planned_through): (&Bound<'py, PyAny>, Option<&Bound<'py, PyAny>>),
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = discovered.py();
    let tracker: Bound<'py, PyAny> = timing_tracker(py)?;
    let started: f64 = monotonic(py)?;
    let mut connection: Option<Bound<'py, PyAny>> = None;
    let result: PyResult<Bound<'py, PyAny>> = (|| {
        let connection_config: Bound<'py, PyAny> = native_field(compiled, "connection_config")?;
        let project: Bound<'py, PyAny> = native_field(compiled, "project")?;
        let node_results_location: Option<(Option<String>, Option<String>)> =
            if python_node_count(discovered)? == 0 {
                None
            } else {
                let (database, schema) =
                    project_target_settings(discovered, &project, (&connection_config, adapter))?
                        .node_locations(adapter)?;
                Some((database.extract()?, schema.extract()?))
            };
        let (opened, reads): (Bound<'py, PyAny>, Bound<'py, PlannerReads>) = match planned_through {
            Some(through) => {
                let on_start: Bound<'py, PyAny> = native_field(hooks, "on_connection_start")?;
                if !on_start.is_none() {
                    on_start.call1((1,))?;
                }
                (
                    py.None().into_bound(py),
                    PlannerReads::passed(through.clone())?,
                )
            }
            None => {
                let opened: Bound<'py, PyAny> = timed(CONNECTING, || {
                    connection_with_hooks(adapter, &connection_config, hooks)
                })?;
                connection = Some(opened.clone());
                let reads = LiveReads::new(adapter, &opened, None).handle(adapter.py())?;
                (opened, reads)
            }
        };
        pipeline_result(
            &PlanningInputs {
                discovered_inputs: discovered,
                adapter,
                connection: &opened,
                reads: &reads,
                node_results_location,
            },
            &project,
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
