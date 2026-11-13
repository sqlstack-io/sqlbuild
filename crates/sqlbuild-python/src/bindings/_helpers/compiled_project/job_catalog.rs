//! The command target's jobs, loaded when a selector names one or `--changed` needs triggered jobs.

use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use pyo3::exceptions::PyValueError;
use pyo3::prelude::{Bound, PyAny, PyResult, Python};
use sqlbuild_analysis::assembly::project::types::ObjectKey;
use sqlbuild_analysis::graph::main::attach_jobs::attach_jobs;
use sqlbuild_analysis::graph::main::selectors_need_jobs::selectors_need_jobs;
use sqlbuild_analysis::graph::models::{JobSelection, ProjectGraph};
use sqlbuild_cli::native_commands::main::effective_target::effective_target;

use crate::bindings::_helpers::compiled_project::snapshot_declarations::discover_declarations;
use crate::bindings::_helpers::native_host::orchestration_declarations::{
    job_selections, triggered_job_names,
};

/// Each job and group's selection, and the names of those a schedule or sensor starts.
type Catalog = (BTreeMap<String, JobSelection>, BTreeSet<String>);

/// The running command's project directory, its `--target` and, once loaded, its jobs.
struct JobScope {
    project_dir: PathBuf,
    selected_target: Option<String>,
    catalog: Option<Catalog>,
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
        Some((catalog, _)) => Cow::Owned(attach_jobs(graph.clone(), catalog)),
        None => Cow::Borrowed(graph),
    })
}

/// The command's job and job group names, loaded on first use; empty outside a command.
pub(crate) fn job_names(py: Python<'_>) -> PyResult<Vec<String>> {
    Ok(job_catalog(py)?
        .map(|(catalog, _)| catalog.into_keys().collect())
        .unwrap_or_default())
}

/// Each job a schedule or sensor starts with its nodes on `graph`, skipping unresolved selections.
pub(crate) fn triggered_jobs(
    py: Python<'_>,
    graph: &ProjectGraph,
) -> PyResult<Vec<(String, BTreeSet<ObjectKey>)>> {
    let Some((catalog, triggered)) = job_catalog(py)? else {
        return Ok(Vec::new());
    };
    if triggered.is_empty() {
        return Ok(Vec::new());
    }
    let mut graph: ProjectGraph = attach_jobs(graph.clone(), catalog);
    let mut resolved = graph.jobs.take().unwrap_or_default();
    Ok(triggered
        .into_iter()
        .filter_map(|name| match resolved.remove(&name) {
            Some(Ok(keys)) => Some((name, keys)),
            _ => None,
        })
        .collect())
}

fn job_catalog(py: Python<'_>) -> PyResult<Option<Catalog>> {
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
    let catalog: Catalog = (
        job_selections(py, &roots, target.as_deref())?,
        triggered_job_names(py, &roots, target.as_deref())?,
    );
    JOB_SCOPE.with(|scope| {
        if let Some(scope) = scope.borrow_mut().as_mut() {
            scope.catalog = Some(catalog.clone());
        }
    });
    Ok(Some(catalog))
}
