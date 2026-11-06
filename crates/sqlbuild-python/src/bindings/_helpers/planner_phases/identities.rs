//! Expected version identities and stale-warning change detection.

use crate::bindings::_helpers::boundary::python_calls::KeywordArguments;
use crate::bindings::_helpers::compiled_views::project_reads;
use crate::bindings::_helpers::planner_changes::detect::scope_model_changes;
use crate::bindings::_helpers::planner_identity::identities::direct_model_version_identities;
use crate::bindings::_helpers::planner_phases::contexts::class_field;
use crate::bindings::_helpers::planner_phases::models::IdentityWork;
use crate::bindings::_helpers::planner_phases::models::PlannerIdentityContext;
use crate::bindings::_helpers::planner_phases::models::PlannerScopeResolution;
use crate::bindings::_helpers::planner_phases::objects::native;
use crate::bindings::_helpers::planner_scope::retained::scope_data;
use crate::bindings::_helpers::records::fields::field as native_field;
use crate::bindings::_helpers::records::fields::mapping_get;
use crate::bindings::_helpers::records::ids::RecordId;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use pyo3::pyfunction;
use pyo3::sync::PyOnceLock;
use pyo3::types::PyDict as KeywordDict;
use pyo3::types::{PyDict, PyDictMethods};
use pyo3::{Py, PyRefMut, Python};
use sqlbuild_analysis::planner_scope::constants::MODEL;
use std::collections::{BTreeMap, HashSet};

static IDENTITY_WORK: PyOnceLock<Py<IdentityWork>> = PyOnceLock::new();

fn identity_work(py: Python<'_>) -> PyResult<PyRefMut<'_, IdentityWork>> {
    Ok(IDENTITY_WORK
        .get_or_try_init(py, || Py::new(py, IdentityWork::default()))?
        .bind(py)
        .borrow_mut())
}

/// Report identity builds and per-model stale-warning change detections, optionally resetting them.
#[pyfunction]
#[pyo3(signature = (*, reset = false))]
pub(crate) fn planner_identity_work_info(
    py: Python<'_>,
    reset: bool,
) -> PyResult<(u64, BTreeMap<String, u64>)> {
    let mut work = identity_work(py)?;
    let info = (work.identity_builds, work.stale_detections.clone());
    if reset {
        *work = IdentityWork::default();
    }
    Ok(info)
}

fn version_identities<'py>(
    project: &Bound<'py, PyAny>,
    scope: Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    identity_work(project.py())?.identity_builds += 1;
    direct_model_version_identities(
        (&project_reads::functions(project)?, &scope),
        native_field(project, "sql_analysis_dialect")?.extract()?,
        (
            &project_reads::seeds(project)?,
            &project.py().None().into_bound(project.py()),
            &native_field(project, "hook_functions")?,
        ),
    )
}

pub(crate) fn build_planner_identity_context<'py>(
    project: &Bound<'py, PyAny>,
    scopes: &Bound<'py, PyAny>,
    include_stale_warning_identities: bool,
) -> PyResult<Bound<'py, PyAny>> {
    let identities = version_identities(
        project,
        class_field(scopes, |s: &PlannerScopeResolution| &s.inspection_scope)?,
    )?;
    let stale = if include_stale_warning_identities {
        version_identities(
            project,
            class_field(scopes, |s: &PlannerScopeResolution| &s.stale_warning_scope)?,
        )?
    } else {
        identities.clone()
    };
    Ok(Bound::new(
        project.py(),
        PlannerIdentityContext {
            version_identities: identities.unbind(),
            stale_warning_identities: stale.unbind(),
        },
    )?
    .into_any())
}

fn reusable_model_names(
    identities: &Bound<'_, PyAny>,
    execution_changes: &Bound<'_, PyAny>,
    query_change_tracking: bool,
) -> PyResult<HashSet<String>> {
    let mut names: HashSet<_> = HashSet::new();
    if !query_change_tracking {
        return Ok(names);
    }
    let execution = class_field(identities, |i: &PlannerIdentityContext| {
        &i.version_identities
    })?;
    let stale = class_field(identities, |i: &PlannerIdentityContext| {
        &i.stale_warning_identities
    })?;
    for name in native_field(execution_changes, "models")?.try_iter()? {
        let name = name?;
        let same = |field: &str| -> PyResult<bool> {
            mapping_get(&execution.getattr(field)?, &name)?
                .eq(mapping_get(&native_field(&stale, field)?, &name)?)
        };
        if same("model_version_hashes")? && same("model_metadata_jsons")? {
            names.insert(name.extract()?);
        }
    }
    Ok(names)
}

#[pyfunction(name = "detect_stale_warning_changes")]
#[pyo3(signature = (**options))]
pub(crate) fn detect_stale_warning_changes_of<'py>(
    py: pyo3::Python<'py>,
    options: Option<Bound<'py, KeywordDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    let args = KeywordArguments::read(
        py,
        "detect_stale_warning_changes",
        options,
        (
            &[
                "project",
                "scopes",
                "snapshot",
                "identities",
                "execution_changes",
            ],
            &[],
        ),
    )?;
    detect_stale_warning_changes(
        (&args.value("project")?, &args.value("scopes")?),
        &args.value("snapshot")?,
        (
            &args.value("identities")?,
            &args.value("execution_changes")?,
        ),
    )
}

/// Changes of stale-warning models whose identities differ from the execution identities.
pub(crate) fn detect_stale_warning_changes<'py>(
    (project, scopes): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
    snapshot: &Bound<'py, PyAny>,
    (identities, execution_changes): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
) -> PyResult<Bound<'py, PyAny>> {
    let py = project.py();
    let reusable = reusable_model_names(
        identities,
        execution_changes,
        native_field(&native_field(project, "settings")?, "query_change_tracking")?.is_truthy()?,
    )?;
    let stale_scope = class_field(scopes, |s: &PlannerScopeResolution| &s.stale_warning_scope)?;
    let stale_scope = scope_data(&stale_scope)?;
    let remaining: HashSet<_> = stale_scope
        .names
        .iter()
        .map(|(_, key)| key)
        .filter(|key| key.0 == MODEL && !reusable.contains(&key.1))
        .cloned()
        .collect();
    let stale_identities = class_field(identities, |i: &PlannerIdentityContext| {
        &i.stale_warning_identities
    })?;
    {
        let mut work = identity_work(py)?;
        for key in &remaining {
            *work.stale_detections.entry(key.1.clone()).or_default() += 1;
        }
    }
    let remaining_changes = scope_model_changes(
        (
            project,
            &stale_scope.with_selected(py, remaining).object(py)?,
            snapshot,
        ),
        Some(true),
        (
            native_field(&stale_identities, "model_version_hashes")?,
            native_field(&stale_identities, "model_metadata_jsons")?,
        ),
    )?;
    let models = PyDict::new(py);
    for (name, change) in native_field(execution_changes, "models")?
        .downcast::<PyDict>()?
        .iter()
    {
        if reusable.contains(&name.extract::<String>()?) {
            models.set_item(name, change)?;
        }
    }
    for (name, change) in remaining_changes.iter() {
        models.set_item(name, change)?;
    }
    native(
        py,
        RecordId::PlannerChangeResults,
        &[
            ("models", models.into_any()),
            ("functions", PyDict::new(py).into_any()),
        ],
    )
}
