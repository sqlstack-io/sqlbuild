//! Jobs for the command's target, as publication keeps them, loaded only when a selector names one.

use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::PathBuf;

use pyo3::exceptions::PyValueError;
use pyo3::prelude::{Bound, PyAny, PyResult, Python};
use sqlbuild_analysis::graph::main::attach_jobs::attach_jobs;
use sqlbuild_analysis::graph::main::selectors_need_jobs::selectors_need_jobs;
use sqlbuild_analysis::graph::models::{JobSelection, ProjectGraph};
use sqlbuild_cli::native_commands::main::effective_target::effective_target;

use crate::bindings::_helpers::compiled_project::snapshot_declarations::discover_declarations;
use crate::bindings::_helpers::native_host::orchestration_declarations::job_selections;

/// The running command's project directory, its `--target` and, once loaded, its jobs.
struct JobScope {
    project_dir: PathBuf,
    selected_target: Option<String>,
    catalog: Option<BTreeMap<String, JobSelection>>,
}

thread_local! {
    /// Per thread, since a command resolves its selectors on the thread that dispatches it.
    static JOB_SCOPE: RefCell<Option<JobScope>> = const { RefCell::new(None) };
}

/// Run one command with jobs selectable from `project_dir`, restoring the enclosing scope after.
pub(crate) fn with_job_scope<T>(
    (project_dir, selected_target): (PathBuf, Option<String>),
    run: impl FnOnce() -> T,
) -> T {
    let previous = JOB_SCOPE.with(|scope| {
        scope.replace(Some(JobScope {
            project_dir,
            selected_target,
            catalog: None,
        }))
    });
    let result: T = run();
    JOB_SCOPE.with(|scope| *scope.borrow_mut() = previous);
    result
}

/// `graph` with the command's jobs attached when `selectors` name a job, otherwise `graph` itself.
pub(crate) fn graph_with_jobs<'g>(
    py: Python<'_>,
    graph: &'g ProjectGraph,
    selectors: &[String],
    also_known: &[&str],
) -> PyResult<Cow<'g, ProjectGraph>> {
    if graph.jobs.is_some() || !selectors_need_jobs(graph, selectors, also_known) {
        return Ok(Cow::Borrowed(graph));
    }
    Ok(match job_catalog(py)? {
        Some(catalog) => Cow::Owned(attach_jobs(graph.clone(), catalog)),
        None => Cow::Borrowed(graph),
    })
}

/// The command's job and job group names, loaded on first use; empty outside a command.
pub(crate) fn job_names(py: Python<'_>) -> PyResult<Vec<String>> {
    Ok(job_catalog(py)?
        .map(|catalog| catalog.into_keys().collect())
        .unwrap_or_default())
}

fn job_catalog(py: Python<'_>) -> PyResult<Option<BTreeMap<String, JobSelection>>> {
    let Some(scope) = JOB_SCOPE.with(|scope| {
        scope.borrow().as_ref().map(|scope| match &scope.catalog {
            Some(catalog) => Ok(catalog.clone()),
            None => Err((scope.project_dir.clone(), scope.selected_target.clone())),
        })
    }) else {
        return Ok(None);
    };
    let (project_dir, selected_target): (PathBuf, Option<String>) = match scope {
        Ok(catalog) => return Ok(Some(catalog)),
        Err(scope) => scope,
    };
    let target: Option<String> =
        effective_target(&project_dir, selected_target).map_err(PyValueError::new_err)?;
    let roots: Vec<(Bound<'_, PyAny>, String)> = discover_declarations(py, &project_dir)?;
    let catalog: BTreeMap<String, JobSelection> = job_selections(py, &roots, target.as_deref())?;
    JOB_SCOPE.with(|scope| {
        if let Some(scope) = scope.borrow_mut().as_mut() {
            scope.catalog = Some(catalog.clone());
        }
    });
    Ok(Some(catalog))
}
