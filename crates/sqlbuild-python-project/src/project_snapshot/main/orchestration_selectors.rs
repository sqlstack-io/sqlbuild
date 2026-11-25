//! Refuse a compile whose job or sensor selectors would not resolve at publish, as `OrchestrationDeclarationError` (D017).

use std::path::Path;

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use sqlbuild_analysis::graph::models::ProjectGraph;
use sqlbuild_discovery::orchestration_files::models::{FilePosition, OrchestrationInventory};
use sqlbuild_python_base::orchestration_reads::main::declared_at::declared_at;
use sqlbuild_python_base::orchestration_reads::main::imported_declarations::imported_declarations;
use sqlbuild_python_base::orchestration_reads::models::AuthoredSensor;
use sqlbuild_python_base::records::main::fields::field::field;
use sqlbuild_python_front::discovery::main::sources::orchestration_declarations::orchestration_declarations;
use sqlbuild_python_front::discovery::main::sources::orchestration_failure::orchestration_failure;

use crate::graph::models::ProjectSelection;
use crate::native_host::main::sensor_selection::sensor_selection;
use crate::project_snapshot::_helpers::snapshot::snapshot_declarations::declared_for;
use crate::project_snapshot::constants::EXECUTED_SUCCESSFULLY;
use crate::project_snapshot::main::python_declarations::python_declarations;

/// Every job and sensor selector declared for the project's target, resolved as publication resolves them; one error names each that fails.
pub fn orchestration_selectors(
    project_dir: &Bound<'_, PyAny>,
    (project, discovered_inputs): (&Bound<'_, PyAny>, &Bound<'_, PyAny>),
    graph: &ProjectGraph,
) -> PyResult<()> {
    let py = project_dir.py();
    let inventory: OrchestrationInventory = orchestration_declarations(project_dir)?;
    if inventory.jobs.is_empty() && inventory.sensors.is_empty() {
        return Ok(());
    }
    let target: String = field(project, "effective_target_name")?
        .extract::<Option<String>>()?
        .unwrap_or_default();
    let selected = ProjectSelection::new(project, discovered_inputs)?;
    let mut failures: Vec<String> = job_failures(&inventory, &target, (graph, &selected));
    if !inventory.sensors.is_empty() {
        let root: std::path::PathBuf = project_dir.extract()?;
        let declarations = python_declarations(py, &root, &inventory)?;
        let sensors: Vec<AuthoredSensor> = imported_declarations(py, declarations)?.sensors;
        failures.extend(sensor_failures(
            (py, &root),
            sensors,
            &target,
            (graph, &selected),
        )?);
    }
    if failures.is_empty() {
        return Ok(());
    }
    Err(orchestration_failure(py, failures.join("\n")))
}

/// `path:line:column: job NAME: error` for each job whose `select` or `exclude` does not resolve.
fn job_failures(
    inventory: &OrchestrationInventory,
    target: &str,
    (graph, selected): (&ProjectGraph, &ProjectSelection),
) -> Vec<String> {
    let tokens = |value: &str| {
        value
            .split_whitespace()
            .map(str::to_owned)
            .collect::<Vec<_>>()
    };
    inventory
        .jobs
        .iter()
        .filter(|(_, job)| declared_for(&job.run.targets, target))
        .filter_map(|(path, job)| {
            let exclude: &str = job.exclude.as_ref().map_or("", |exclude| &exclude.value);
            let error = selected
                .run_keys(graph, &tokens(&job.select.value), &tokens(exclude))
                .err()?;
            let FilePosition { line, column } = job.select.position;
            Some(format!(
                "{path}:{line}:{column}: job {}: {error}",
                job.run.name.value
            ))
        })
        .collect()
}

/// `path:line: sensor NAME: error` for each sensor whose condition selectors do not resolve.
fn sensor_failures(
    (py, project_dir): (Python<'_>, &Path),
    sensors: Vec<AuthoredSensor>,
    target: &str,
    graph: (&ProjectGraph, &ProjectSelection),
) -> PyResult<Vec<String>> {
    let mut failures: Vec<String> = Vec::new();
    for sensor in sensors {
        let (Some(name), Some(when)) = (sensor.name, sensor.when) else {
            continue;
        };
        if !sensor.targets.is_empty() && !sensor.targets.iter().any(|value| value == target) {
            continue;
        }
        let site: String = declared_at(project_dir, sensor.declared_at)?
            .map_or_else(String::new, |at| format!("{}:{}: ", at.path, at.line));
        let mut checks: Vec<(Vec<String>, String)> = Vec::new();
        if let Some(select) = when.select {
            checks.push((
                select.split_whitespace().map(str::to_owned).collect(),
                when.kind,
            ));
        }
        if let Some(selectors) = when.executed_successfully {
            checks.push((selectors, EXECUTED_SUCCESSFULLY.to_owned()));
        }
        for (selectors, field_name) in checks {
            if let Err(error) = sensor_selection(graph, &selectors, &field_name) {
                failures.push(format!("{site}sensor {name}: {}", error.value(py)));
            }
        }
    }
    Ok(failures)
}
