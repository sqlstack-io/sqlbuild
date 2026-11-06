//! Plan output assembly: storage policies, warnings and direct-mode metadata.

use crate::bindings::_helpers::boundary::python_calls::keywords;
use crate::bindings::_helpers::native_host::model_lookup::ModelLookup;
use crate::bindings::_helpers::planner_entries::models::PlanOutputInputs;
use crate::bindings::_helpers::planner_entries::plan_output::plan_output_fields;
use crate::bindings::_helpers::planner_identity::staleness::build_stale_out_of_selection_warnings;
use crate::bindings::_helpers::planner_migrations::fingerprint::with_migration_fingerprints;
use crate::bindings::_helpers::planner_phases::contexts::class_field;
use crate::bindings::_helpers::planner_phases::models::PlannerIdentityContext;
use crate::bindings::_helpers::planner_phases::models::PlannerScopeResolution;
use crate::bindings::_helpers::planner_phases::models::{
    PlanOutputContext, PlanOutputFields, PlanWarningContext, PlannedModelEntries,
    StoragePolicyContext,
};
use crate::bindings::_helpers::planner_phases::objects::object;
use crate::bindings::_helpers::planner_phases::objects::{
    native, replace, sorted_names, strings, text, tuple,
};
use crate::bindings::_helpers::planner_phases::retention::{plan_retention, plan_table_types};
use crate::bindings::_helpers::planner_phases::runtime::runtime_field;
use crate::bindings::_helpers::planner_phases::runtime::warehouse_field;
use crate::bindings::_helpers::planner_scope::retained::scope_data;
use crate::bindings::_helpers::planner_warehouse::source_freshness::source_freshness_unknown_warnings;
use crate::bindings::_helpers::records::construct::{enum_member, new_record_with};
use crate::bindings::_helpers::records::fields::field as native_field;
use crate::bindings::_helpers::records::fields::mapping_items;
use crate::bindings::_helpers::records::ids::RecordId;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyBool, PyDict, PyDictMethods, PyTuple};
use sqlbuild_analysis::planner_scope::constants::MODEL;

fn concat<'py>(py: Python<'py>, parts: &[Bound<'py, PyAny>]) -> PyResult<Bound<'py, PyAny>> {
    let mut items: Vec<_> = Vec::new();
    for part in parts {
        for item in part.try_iter()? {
            items.push(item?);
        }
    }
    tuple(py, items)
}

impl<'py> PlanOutputFields<'py> {
    fn field(&self, name: &str) -> PyResult<Bound<'py, PyAny>> {
        self.fields
            .iter()
            .find(|(field, _)| *field == name)
            .map(|(_, value)| value.clone())
            .ok_or_else(|| pyo3::exceptions::PyKeyError::new_err(name.to_owned()))
    }

    fn set(&mut self, name: &'static str, value: Bound<'py, PyAny>) {
        match self.fields.iter_mut().find(|(field, _)| *field == name) {
            Some(existing) => existing.1 = value,
            None => self.fields.push((name, value)),
        }
    }

    /// The `PlanOutput` hand-off, built once from every field the phases set.
    pub(crate) fn build(mut self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        if !self.metadata.is_empty() {
            let metadata = PyDict::new(py);
            for (name, value) in &self.metadata {
                metadata.set_item(*name, value)?;
            }
            self.set("metadata", metadata.into_any());
        }
        new_record_with(py, RecordId::PlanOutput, &self.fields)
    }
}

/// The plan output's base fields from the planned entries, relations and source freshness.
pub(crate) fn base_plan_output<'py>(
    PlanOutputContext {
        runtime,
        warehouse,
        identities,
        overrides,
    }: PlanOutputContext<'_, 'py>,
    PlannedModelEntries {
        execution_scope,
        reconciliation,
        entries,
        source_freshness,
    }: PlannedModelEntries<'_, 'py>,
    planned_sql_tests: &Bound<'py, PyAny>,
) -> PyResult<PlanOutputFields<'py>> {
    let py = runtime.py();
    let version = class_field(identities, |i: &PlannerIdentityContext| {
        &i.version_identities
    })?;
    let extras = native(
        py,
        RecordId::PlanOutputExtras,
        &[
            (
                "seed_version_hashes",
                native_field(&version, "seed_version_hashes")?,
            ),
            (
                "seed_metadata_jsons",
                native_field(&version, "seed_metadata_jsons")?,
            ),
            ("planned_sql_tests", planned_sql_tests.clone()),
        ],
    )?;
    let mut output = PlanOutputFields {
        fields: plan_output_fields(PlanOutputInputs {
            project: runtime_field(runtime, |runtime| &runtime.project)?,
            adapter: runtime_field(runtime, |runtime| &runtime.adapter)?,
            scope: execution_scope.clone(),
            snapshot: warehouse_field(warehouse, |warehouse| &warehouse.snapshot)?,
            relations: warehouse_field(warehouse, |warehouse| &warehouse.inspection_relations)?,
            changes: reconciliation.changes.clone(),
            model_entries: entries.entries,
            model_warnings: entries.warnings,
            reload_sources: native_field(overrides, "reload_sources")?,
            extras,
        })?,
        metadata: Vec::new(),
    };
    output.set("source_freshness", source_freshness.clone());
    output.set(
        "cursor_overrides",
        native_field(overrides, "cursor_overrides")?,
    );
    Ok(output)
}

/// Attach near-match column rename hints to the model entries they describe.
pub(crate) fn with_column_rename_hints<'py>(
    entries: &Bound<'py, PyAny>,
    hints: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py = entries.py();
    let by_model = PyDict::new(py);
    for hint in hints.try_iter()? {
        let hint = hint?;
        let name = native_field(&hint, "model_name")?;
        if let Some(existing) = by_model.get_item(&name)? {
            existing.call_method1("append", (hint,))?;
        } else {
            by_model.set_item(name, pyo3::types::PyList::new(py, [hint])?)?;
        }
    }
    let mut attached: Vec<_> = Vec::new();
    for entry in entries.try_iter()? {
        let entry = entry?;
        match by_model.get_item(native_field(&entry, "name")?)? {
            Some(found) => attached.push(replace(
                &entry,
                &[(
                    "column_rename_hints",
                    PyTuple::new(py, found.try_iter()?.collect::<PyResult<Vec<_>>>()?)?.into_any(),
                )],
            )?),
            None => attached.push(entry),
        }
    }
    tuple(py, attached)
}

/// Add migration fingerprints, rename hints, migration, table-type and retention entries.
pub(crate) fn add_storage_policies<'py>(
    mut output: PlanOutputFields<'py>,
    StoragePolicyContext {
        runtime,
        warehouse,
        scopes,
        policies,
    }: StoragePolicyContext<'_, 'py>,
) -> PyResult<PlanOutputFields<'py>> {
    let py = runtime.py();
    let mut entries = output.field("model_entries")?;
    if native_field(policies, "record_migration_fingerprints")?.is_truthy()? {
        let scope = class_field(scopes, |s: &PlannerScopeResolution| &s.selected_scope)?;
        entries = object(
            py,
            with_migration_fingerprints(
                &entries,
                &ModelLookup::Native(scope_data(&scope)?),
                &warehouse_field(warehouse, |warehouse| &warehouse.migration_fingerprints)?,
                &native_field(runtime, "adapter")?.call_method0("sql_analysis_dialect")?,
            )?,
        )?;
    }
    let entries = with_column_rename_hints(
        &entries,
        &warehouse_field(warehouse, |warehouse| &warehouse.column_rename_hints)?,
    )?;
    let migration_entries = warehouse_field(warehouse, |warehouse| &warehouse.migration_entries)?;
    let column_entries =
        warehouse_field(warehouse, |warehouse| &warehouse.column_migration_entries)?;
    let old_name_entries =
        warehouse_field(warehouse, |warehouse| &warehouse.old_name_view_entries)?;
    let old_name_views = warehouse_field(warehouse, |warehouse| &warehouse.old_name_views)?;
    let warnings = concat(
        py,
        &[
            warehouse_field(warehouse, |warehouse| &warehouse.migration_warnings)?,
            output.field("warnings")?,
        ],
    )?;
    let selected = class_field(scopes, |s: &PlannerScopeResolution| &s.selected_scope)?;
    let table_types = plan_table_types(runtime, warehouse, &selected)?;
    let retention = plan_retention(runtime, warehouse, &selected)?;
    output.set("model_entries", entries);
    output.set("migration_entries", migration_entries);
    output.set("column_migration_entries", column_entries);
    output.set("old_name_view_entries", old_name_entries);
    output.set("old_name_views", old_name_views);
    output.set("warnings", warnings);
    output.set("table_type_entries", table_types);
    output.set("retention_entries", retention);
    Ok(output)
}

/// Add unknown source freshness warnings and, with selection diagnostics, stale-selection warnings.
pub(crate) fn add_plan_warnings<'py>(
    mut output: PlanOutputFields<'py>,
    PlanWarningContext {
        scopes,
        warehouse,
        identities,
        stale_warning_changes,
    }: PlanWarningContext<'_, 'py>,
    (source_freshness, policies): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
) -> PyResult<PlanOutputFields<'py>> {
    let py = scopes.py();
    let freshness = source_freshness_unknown_warnings(source_freshness)?;
    if freshness.is_truthy()? {
        let warnings = concat(py, &[output.field("warnings")?, freshness])?;
        output.set("warnings", warnings);
    }
    if !native_field(policies, "selection_diagnostics")?.is_truthy()? {
        return Ok(output);
    }
    let stale = object(
        py,
        build_stale_out_of_selection_warnings(
            py,
            Some(keywords(
                py,
                &[
                    (
                        "original_scope",
                        class_field(scopes, |s: &PlannerScopeResolution| &s.stale_warning_scope)?,
                    ),
                    (
                        "execution_scope",
                        class_field(scopes, |s: &PlannerScopeResolution| &s.inspection_scope)?,
                    ),
                    ("changes", stale_warning_changes.clone()),
                    (
                        "snapshot",
                        warehouse_field(warehouse, |warehouse| &warehouse.snapshot)?,
                    ),
                    (
                        "version_identities",
                        class_field(identities, |i: &PlannerIdentityContext| {
                            &i.stale_warning_identities
                        })?,
                    ),
                    ("source_freshness", source_freshness.clone()),
                    (
                        "include_sources",
                        PyBool::new(py, false).to_owned().into_any(),
                    ),
                    (
                        "model_changes_complete",
                        PyBool::new(py, true).to_owned().into_any(),
                    ),
                ],
            )?),
        )?,
    )?;
    if stale.is_truthy()? {
        let warnings = concat(py, &[output.field("warnings")?, stale])?;
        output.set("warnings", warnings);
    }
    Ok(output)
}

/// The unselected models the selected scope reads, sorted.
pub(crate) fn unselected_upstream_models(scopes: &Bound<'_, PyAny>) -> PyResult<Vec<String>> {
    let selected = class_field(scopes, |s: &PlannerScopeResolution| &s.selected_scope)?;
    let selected = crate::bindings::_helpers::planner_scope::retained::scope_data(&selected)?;
    let mut names: Vec<String> = Vec::new();
    for (key, deps) in &selected.upstream {
        if !selected.selected.contains(key) {
            continue;
        }
        for dep in deps {
            if dep.0 == MODEL && !selected.selected.contains(dep) {
                names.push(dep.1.clone());
            }
        }
    }
    names.sort();
    names.dedup();
    Ok(names)
}

/// Note unselected upstreams deferred to pre-migration names, and each lookup in `failures`.
pub(crate) fn add_deferred_origin_notes<'py>(
    mut output: PlanOutputFields<'py>,
    (project, scopes): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
    (deferral, failures): (&Bound<'py, PyAny>, &[(String, String)]),
) -> PyResult<PlanOutputFields<'py>> {
    let py = project.py();
    let locations = native_field(deferral, "deferred_locations")?;
    if locations.is_none() {
        return Ok(output);
    }
    let names: Vec<String> = unselected_upstream_models(scopes)?;
    let warning = |name: &str, message: &str| -> PyResult<Bound<'py, PyAny>> {
        native(
            py,
            RecordId::PlanWarning,
            &[
                ("model_name", text(py, name)),
                (
                    "severity",
                    enum_member(py, RecordId::WarningSeverity, "WARNING")?,
                ),
                ("message", text(py, message)),
            ],
        )
    };
    let mut notes: Vec<Bound<'py, PyAny>> = Vec::new();
    for model in native_field(project, "models")?.try_iter()? {
        let model = model?;
        let name: String = native_field(&model, "name")?.extract()?;
        if names.binary_search(&name).is_err() {
            continue;
        }
        let Some(location) = locations.downcast::<PyDict>()?.get_item(&name)? else {
            continue;
        };
        if native_field(&location, "name")?
            .eq(native_field(&native_field(&model, "destination")?, "name")?)?
        {
            continue;
        }
        notes.push(warning(
            &name,
            &format!(
                "deferred to pre-migration name {}: the deferred target has not built model '{name}' under its current name yet",
                native_field(&location, "qualified_name")?
            ),
        )?);
    }
    for (name, message) in failures {
        notes.push(warning(name, message)?);
    }
    if !notes.is_empty() {
        let warnings = concat(py, &[output.field("warnings")?, tuple(py, notes)?])?;
        output.set("warnings", warnings);
    }
    Ok(output)
}

fn source_names<'py>(items: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let mut names: Vec<_> = Vec::new();
    for item in items.try_iter()? {
        names.push(native_field(&item?, "source_name")?.extract::<String>()?);
    }
    sorted_names(items.py(), names)
}

fn age_names<'py>(statuses: &Bound<'py, PyAny>, wanted: &str) -> PyResult<Bound<'py, PyAny>> {
    let mut names: Vec<_> = Vec::new();
    for item in mapping_items(statuses)?.try_iter()? {
        let (identity, status): (Bound<'py, PyAny>, Bound<'py, PyAny>) = item?.extract()?;
        if status.eq(wanted)? {
            names.push(native_field(&identity, "source_name")?.extract::<String>()?);
        }
    }
    sorted_names(statuses.py(), names)
}

fn freshness_metadata<'py>(source_freshness: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py = source_freshness.py();
    let propagation = native_field(source_freshness, "propagation")?;
    let propagated = |field: &str| -> PyResult<Bound<'py, PyAny>> {
        if propagation.is_none() {
            return tuple(py, Vec::new());
        }
        sorted_names(py, strings(&native_field(&propagation, field)?)?)
    };
    let mut details: Vec<_> = Vec::new();
    for unknown in native_field(source_freshness, "unknown_sources")?
        .call_method0("values")?
        .try_iter()?
    {
        let unknown = unknown?;
        let detail = PyDict::new(py);
        detail.set_item("source_name", native_field(&unknown, "source_name")?)?;
        detail.set_item(
            "reason",
            native_field(&native_field(&unknown, "reason")?, "value")?,
        )?;
        detail.set_item("message", native_field(&unknown, "message")?)?;
        details.push(detail.into_any());
    }
    let statuses = native_field(source_freshness, "age_statuses")?;
    let metadata = PyDict::new(py);
    metadata.set_item(
        "observed_source_names",
        source_names(&native_field(source_freshness, "observed_records")?)?,
    )?;
    metadata.set_item(
        "changed_source_names",
        source_names(&native_field(source_freshness, "changed_identities")?)?,
    )?;
    metadata.set_item(
        "unchanged_source_names",
        source_names(&native_field(source_freshness, "unchanged_identities")?)?,
    )?;
    metadata.set_item(
        "unknown_source_names",
        sorted_names(
            py,
            strings(&native_field(source_freshness, "unknown_source_names")?)?,
        )?,
    )?;
    metadata.set_item("unknown_source_details", tuple(py, details)?)?;
    metadata.set_item("age_warning_source_names", age_names(&statuses, "warn")?)?;
    metadata.set_item("age_error_source_names", age_names(&statuses, "error")?)?;
    metadata.set_item("stale_model_names", propagated("stale_model_names")?)?;
    metadata.set_item("blocked_model_names", propagated("blocked_model_names")?)?;
    Ok(metadata.into_any())
}

/// Add the direct-mode source freshness, remaining stale models and selection diagnostics metadata.
pub(crate) fn add_plan_metadata<'py>(
    mut output: PlanOutputFields<'py>,
    source_freshness: &Bound<'py, PyAny>,
    policies: &Bound<'py, PyAny>,
) -> PyResult<PlanOutputFields<'py>> {
    let py = source_freshness.py();
    let diagnostics = PyDict::new(py);
    diagnostics.set_item("mode", text(py, "direct"))?;
    diagnostics.set_item("enabled", native_field(policies, "selection_diagnostics")?)?;
    output.metadata.push((
        "direct_source_freshness",
        freshness_metadata(source_freshness)?,
    ));
    output.metadata.push((
        "direct_remaining_stale_model_names",
        sorted_names(py, Vec::new())?,
    ));
    output
        .metadata
        .push(("selection_diagnostics", diagnostics.into_any()));
    Ok(output)
}
