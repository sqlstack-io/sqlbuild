//! `sqb scenario test --local --sync-snapshots` and `--refresh`: capture stale snapshots first.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyString, PyTuple, PyTupleMethods};
use sqlbuild_core::cli_text::main::format_summary_footer::format_summary_footer;
use sqlbuild_core::cli_text::models::SummaryCount;
use sqlbuild_core::scenario_command::main::snapshot_capture_warning::snapshot_capture_warning;
use sqlbuild_core::scenario_command::models::ScenarioNamespace;
use sqlbuild_host::entries::models::ExecutorHost;

use crate::bindings::_helpers::scenario_command::conversions::succeeded;
use crate::bindings::_helpers::scenario_command::presentation::{
    CompletionKind, RelayContext, SectionStatus, begin_section, connection_hooks, progress_relay,
};
use crate::bindings::_helpers::scenario_command::setup::{
    CompileInputs, boolean, capture_limits, capture_settings, compile, flush, optional_text,
    project_connection_config, selected_scenarios, text, write,
};
use crate::bindings::_helpers::scenario_command::test_command::TestRun;
use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::boundary::main::keywords::keywords;
use sqlbuild_python_base::native_host::main::host_entry::host_entry;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_output::build_progress::models::PlanningProgressReporter;
use sqlbuild_python_output::result_output::main::results::format_scenario_snapshot_execution_json::format_scenario_snapshot_execution_json;
use sqlbuild_python_output::result_output::main::results::write_execution_json_output::write_execution_json_output;

/// The outcome of syncing local snapshots: its exit code and capture results.
pub(crate) struct LocalSync<'py> {
    pub(crate) exit_code: i32,
    results: Bound<'py, PyTuple>,
    refresh: bool,
}

impl LocalSync<'_> {
    /// Write the snapshot capture JSON document and return the failing exit code.
    pub(crate) fn write_failure_json(
        &self,
        request: &Bound<'_, PyAny>,
        namespace: &ScenarioNamespace,
    ) -> PyResult<i32> {
        let py: Python<'_> = request.py();
        let payload: Bound<'_, PyAny> = PyString::new(
            py,
            &format_scenario_snapshot_execution_json(
                py,
                Some(keywords(
                    py,
                    &[
                        (
                            "run_namespace",
                            optional_text(py, namespace.value.as_deref()),
                        ),
                        ("namespace_source", text(py, &namespace.source)),
                        ("results", self.results.clone().into_any()),
                        ("refresh", boolean(py, self.refresh)),
                    ],
                )?),
            )?,
        )
        .into_any();
        write_execution_json_output(
            py,
            payload,
            native_field(request, "json_output")?.extract()?,
            native_field(request, "json_output_path")?,
        )?;
        Ok(self.exit_code)
    }
}

fn label(prefix: &str, suffix: &str, value: usize) -> SummaryCount {
    SummaryCount {
        label: format!("{prefix}_{suffix}"),
        value: i64::try_from(value).unwrap_or(i64::MAX),
    }
}

/// Capture the selected scenarios whose local snapshots are missing or stale.
pub(crate) fn sync_local_snapshots<'py>(
    request: &Bound<'py, PyAny>,
    run: &TestRun<'py>,
    (local_pipeline, local_scenarios): (&Bound<'py, PyAny>, &Bound<'py, PyTuple>),
) -> PyResult<LocalSync<'py>> {
    let py: Python<'py> = request.py();
    let refresh: bool = native_field(request, "refresh")?.is_truthy()?;
    let project_connection: Bound<'py, PyAny> = project_connection_config(&run.project)?;
    let project_name: Bound<'py, PyAny> = native_field(
        &native_field(&run.project.discovered, "project_config")?,
        "name",
    )?;
    let names: Vec<String> = call_with(
        &host_entry(py, ExecutorHost::SelectScenarioSnapshotCaptureCandidates)?,
        &[
            ("project_dir", run.project.project_dir.clone()),
            ("pipeline_result", local_pipeline.clone()),
            ("scenarios", local_scenarios.clone().into_any()),
            ("adapter", run.adapter.clone()),
            ("project_name", project_name.clone()),
            ("capture_adapter", text(py, &run.project_adapter_name)),
            ("capture_dialect", text(py, &run.capture_dialect)),
            ("refresh", boolean(py, refresh)),
            (
                "source_lexical_syntax",
                native_field(&run.project_adapter, "sql_lexical_syntax")?,
            ),
        ],
    )?
    .extract()?;
    if names.is_empty() {
        write(&run.stream, "\nSnapshots are fresh.\n")?;
        flush(&run.stream)?;
        return Ok(LocalSync {
            exit_code: 0,
            results: PyTuple::empty(py),
            refresh,
        });
    }
    let planning: Bound<'py, PyAny> = Bound::new(
        py,
        PlanningProgressReporter::new(py, &run.stream, run.style.use_color)?,
    )?
    .into_any();
    let compiled: PyResult<Bound<'py, PyAny>> = compile(
        &run.project,
        CompileInputs {
            adapter: &run.project_adapter,
            options: &[
                (
                    "no_sql_validation",
                    native_field(request, "no_sql_validation")?,
                ),
                ("source_deferral_enabled", boolean(py, false)),
                ("connection_config", project_connection.clone()),
            ],
            planning: &planning,
        },
    );
    let finished: PyResult<Bound<'py, PyAny>> = planning.call_method0("finish");
    let pipeline: Bound<'py, PyAny> = compiled?;
    finished?;
    let scenarios: Bound<'py, PyTuple> =
        selected_scenarios(&pipeline, (&names, &[], &run.project.project_dir))?;
    let force: bool =
        native_field(&native_field(request, "limit_inputs")?, "force")?.is_truthy()?;
    let section: SectionStatus<'py> = begin_section(
        &run.stream,
        (
            &format!(
                "Snapshot {} ({} selected)",
                if refresh { "Refresh" } else { "Sync" },
                scenarios.len()
            ),
            &format!("{}\n\n", snapshot_capture_warning(force)),
            "Capturing snapshots...".to_owned(),
        ),
        run.style,
    )?;
    let hooks: Bound<'py, PyAny> =
        connection_hooks(&run.stream, (&run.project_adapter_name, run.style))?;
    let relay = progress_relay(
        &run.stream,
        &section,
        RelayContext {
            project_dir: &run.project.project_dir,
            adapter: &run.project_adapter,
            kind: CompletionKind::Sync { refresh },
            style: run.style,
        },
    )?;
    let settings: Bound<'py, PyAny> = capture_settings(
        py,
        (&run.project_adapter_name, &run.capture_dialect, false),
        capture_limits(request, &run.project)?,
    )?;
    let results: Bound<'py, PyTuple> = PyTuple::new(
        py,
        call_with(
            &host_entry(py, ExecutorHost::RunScenarioCapturePipeline)?,
            &[
                ("project_dir", run.project.project_dir.clone()),
                ("pipeline_result", pipeline),
                ("scenarios", scenarios.into_any()),
                ("connection_config", project_connection),
                ("adapter", run.project_adapter.clone()),
                ("project_name", project_name),
                ("settings", settings),
                ("connection_hooks", hooks),
                (
                    "on_scenario_start",
                    native_field(&relay, "on_scenario_start")?,
                ),
                (
                    "on_scenario_complete",
                    native_field(&relay, "on_scenario_complete")?,
                ),
            ],
        )?
        .try_iter()?
        .collect::<PyResult<Vec<_>>>()?,
    )?;
    section.status.call_method0("close")?;
    let mut passed: usize = 0;
    for result in results.iter() {
        if succeeded(&result)? {
            passed += 1;
        }
    }
    let failed: usize = results.len() - passed;
    let prefix: &str = if refresh { "REFRESH" } else { "SYNC" };
    write(
        &run.stream,
        &format!(
            "\n{}\n",
            format_summary_footer(
                run.style,
                &[label(prefix, "PASS", passed), label(prefix, "FAIL", failed)],
                None
            )
        ),
    )?;
    flush(&run.stream)?;
    Ok(LocalSync {
        exit_code: i32::from(failed != 0),
        results,
        refresh,
    })
}
