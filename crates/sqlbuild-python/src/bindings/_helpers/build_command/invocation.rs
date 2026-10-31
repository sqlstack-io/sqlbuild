//! Resolve discovery, target, adapter, limits, connection and reporters for one build.

use std::sync::Arc;
use std::time::Instant;

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use sqlbuild_config::effective::models::EffectiveConcurrency;
use sqlbuild_core::build_command::errors::BuildCommandError;
use sqlbuild_core::build_command::main::duration_limit_error::duration_limit_error;
use sqlbuild_core::build_command::models::DurationLimit;

use crate::bindings::_helpers::boundary::python_calls::imported;
use crate::bindings::_helpers::boundary::python_calls::keywords;
use crate::bindings::_helpers::build_command::policies::build_command_error;
use crate::bindings::_helpers::build_command::support::{boolean, or_none, text};
use crate::bindings::_helpers::build_progress::progress_reporters::ConnectionProgressReporter;
use crate::bindings::_helpers::build_progress::progress_reporters::PlanningProgressReporter;
use crate::bindings::_helpers::command_runtime::adapters::resolve_cli_adapter;
use crate::bindings::_helpers::command_runtime::concurrency::resolve_concurrency;
use crate::bindings::_helpers::command_runtime::connection::resolve_project_connection_config;
use crate::bindings::_helpers::compile_attachment::effective_config::build_effective_settings_config;
use crate::bindings::_helpers::compile_driver::discovery::discover_project_inputs;
use crate::bindings::_helpers::compiled_project::snapshot_execution::WorkerBuild;
use crate::bindings::_helpers::contracts::spec_resolution::resolve_effective_adapter_name;
use crate::bindings::_helpers::contracts::spec_resolution::resolve_target_config;
use crate::bindings::_helpers::contracts::spec_resolution::resolve_target_name;
use crate::bindings::_helpers::presentation_text::terminal::stdout_supports_color;
use crate::bindings::_helpers::records::construct::{new_record, new_record_with};
use crate::bindings::_helpers::records::fields::field as native_field;
use crate::bindings::_helpers::records::ids::RecordId;

const DEFER_CONFLICT_CODE: &str = "C408";

/// One build's request, resolved invocation and output settings.
pub(crate) struct BuildContext<'py> {
    pub(crate) request: Bound<'py, PyAny>,
    pub(crate) invocation: Bound<'py, PyAny>,
    pub(crate) stream: Bound<'py, PyAny>,
    pub(crate) use_color: bool,
    pub(crate) verbose: bool,
    pub(crate) started: Instant,
    pub(crate) tracker: Bound<'py, PyAny>,
    pub(crate) worker: Option<Arc<WorkerBuild>>,
}

impl<'py> BuildContext<'py> {
    pub(crate) fn py(&self) -> Python<'py> {
        self.request.py()
    }

    pub(crate) fn request_flag(&self, name: &str) -> PyResult<bool> {
        native_field(&self.request, name)?.is_truthy()
    }

    pub(crate) fn get(&self, name: &str) -> PyResult<Bound<'py, PyAny>> {
        native_field(&self.invocation, name)
    }

    /// `--concurrency`, else the selected target's `concurrency`, else `[settings]`.
    pub(crate) fn concurrency(&self) -> PyResult<EffectiveConcurrency> {
        resolve_concurrency(
            &self.request.getattr("concurrency")?,
            &self.get("discovered_inputs")?,
            &native_field(&self.request, "selected_target")?,
        )
    }
}

fn target_config<'py>(
    discovered: &Bound<'py, PyAny>,
    target_name: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = discovered.py();
    resolve_target_config(
        py,
        &native_field(discovered, "project_config")?,
        &native_field(discovered, "local_config")?,
        &target_name.clone(),
    )
}

fn execution_limits<'py>(
    discovered: &Bound<'py, PyAny>,
    target_name: &Bound<'py, PyAny>,
    (adapter_name, adapter): (&str, &Bound<'py, PyAny>),
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = discovered.py();
    let limits: Bound<'py, PyAny> = if target_name.is_none() {
        new_record(py, RecordId::ExecutionLimitsConfig, (), None)?
    } else {
        native_field(&target_config(discovered, target_name)?, "execution_limits")?
    };
    let limit: DurationLimit = DurationLimit {
        target_name: target_name.extract()?,
        adapter_name: adapter_name.to_owned(),
        configures_max_duration: !native_field(&limits, "max_duration")?.is_none(),
        max_duration_seconds: native_field(&limits, "max_duration_seconds")?.extract()?,
        adapter_limit_seconds: native_field(adapter, "execution_duration_limit_seconds")?
            .extract()?,
        remediation: native_field(&limits, "remediation")?.extract()?,
    };
    if let Some(error) = duration_limit_error(&limit) {
        return Err(build_command_error(py, error));
    }
    Ok(limits)
}

/// Discover the project and resolve everything the build reads before compiling.
pub(crate) fn resolve_build_context<'py>(
    request: &Bound<'py, PyAny>,
    tracker: &Bound<'py, PyAny>,
    started: Instant,
) -> PyResult<BuildContext<'py>> {
    let py: Python<'py> = request.py();
    let project_dir: Bound<'py, PyAny> = or_none(native_field(request, "project_dir")?, || {
        imported(py, "pathlib", "Path")?.call_method0("cwd")
    })?;
    let discovered: Bound<'py, PyAny> =
        or_none(native_field(request, "discovered_inputs")?, || {
            discover_project_inputs(
                py,
                Some(keywords(
                    py,
                    &[
                        ("project_dir", project_dir.clone()),
                        ("extract_output_column_locations", boolean(py, false)),
                    ],
                )?),
            )
        })?;
    let selected_target: Bound<'py, PyAny> = native_field(request, "selected_target")?;
    let target_name: Bound<'py, PyAny> = resolve_target_name(
        py,
        &native_field(&discovered, "project_config")?,
        &native_field(&discovered, "local_config")?,
        &selected_target.clone(),
    )
    .and_then(|value| pyo3::IntoPyObjectExt::into_bound_py_any(value, py))?;
    let defer_clone_from: Bound<'py, PyAny> =
        or_none(native_field(request, "defer_clone_from")?, || {
            if target_name.is_none() {
                return Ok(target_name.clone());
            }
            native_field(
                &target_config(&discovered, &target_name)?,
                "defer_clone_from",
            )
        })?;
    if !native_field(request, "defer_to")?.is_none() && !defer_clone_from.is_none() {
        return Err(build_command_error(
            py,
            BuildCommandError::new(
                Some(DEFER_CONFLICT_CODE),
                "--defer-clone-from cannot be used with --defer-to".to_owned(),
                None,
            ),
        ));
    }
    let adapter_name: String = resolve_effective_adapter_name(
        &native_field(&discovered, "project_config")?,
        &native_field(&discovered, "local_config")?,
    )?
    .extract()?;
    let adapter: Bound<'py, PyAny> = resolve_cli_adapter(&text(py, &adapter_name), &project_dir)?;
    let limits: Bound<'py, PyAny> =
        execution_limits(&discovered, &target_name, (&adapter_name, &adapter))?;
    let connection_config: Bound<'py, PyAny> = resolve_project_connection_config(
        py,
        Some(keywords(
            py,
            &[
                ("discovered_inputs", discovered.clone()),
                ("project_dir", project_dir.clone()),
                ("selected_target", selected_target),
                ("cli_vars", native_field(request, "cli_vars")?),
            ],
        )?),
    )?;
    let machine_output: bool = native_field(request, "json_output")?.is_truthy()?;
    let debug: bool = native_field(request, "debug")?.is_truthy()?;
    let use_color: bool = !native_field(request, "no_color")?.is_truthy()?
        && !machine_output
        && stdout_supports_color(py)?;
    let stream: Bound<'py, PyAny> = py.import("sys")?.getattr(if debug || machine_output {
        "stderr"
    } else {
        "stdout"
    })?;
    let connection_progress: Bound<'py, PyAny> = ConnectionProgressReporter::new(
        py,
        Some(keywords(
            py,
            &[
                ("adapter_name", text(py, &adapter_name)),
                ("stream", stream.clone()),
                ("use_color", boolean(py, use_color)),
            ],
        )?),
    )
    .and_then(|value| pyo3::Bound::new(py, value))
    .map(pyo3::Bound::into_any)?;
    let planning_progress: Bound<'py, PyAny> = PlanningProgressReporter::new(
        py,
        &stream.clone(),
        boolean(py, use_color).extract::<bool>()?,
    )
    .and_then(|value| pyo3::Bound::new(py, value))
    .map(pyo3::Bound::into_any)?;
    let load_sources: Bound<'py, PyAny> = native_field(request, "load_sources")?;
    let should_load_sources: bool = native_field(request, "reload_sources")?.is_truthy()?
        || if load_sources.is_none() {
            native_field(
                &build_effective_settings_config(&discovered.clone())?,
                "auto_load_sources",
            )?
            .is_truthy()?
        } else {
            load_sources.is_truthy()?
        };
    let invocation: Bound<'py, PyAny> = new_record_with(
        py,
        RecordId::BuildInvocation,
        &[
            ("effective_project_dir", project_dir),
            ("discovered_inputs", discovered),
            ("effective_defer_clone_from", defer_clone_from),
            ("adapter_name", text(py, &adapter_name)),
            ("adapter", adapter),
            ("connection_config", connection_config),
            ("use_color", boolean(py, use_color)),
            ("progress_stream", stream.clone()),
            ("connection_progress", connection_progress),
            ("planning_progress", planning_progress),
            ("should_load_sources", boolean(py, should_load_sources)),
            ("effective_target_name", target_name),
            ("execution_limits", limits),
        ],
    )?;
    Ok(BuildContext {
        request: request.clone(),
        invocation,
        stream,
        use_color,
        verbose: native_field(request, "verbose")?.is_truthy()? || debug,
        started,
        tracker: tracker.clone(),
        worker: None,
    })
}
