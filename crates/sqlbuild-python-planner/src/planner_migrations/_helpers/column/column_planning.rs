//! Host column migration planning while Python owns warehouse inspection.

use crate::planner_migrations::_helpers::column::column_objects::replace;
use crate::planner_migrations::_helpers::column::known_input_columns::KnownInputColumns;
use crate::planner_migrations::models::{
    ColumnMigrationInputs, ColumnMigrationPlanning, MigrationPhase,
};
use crate::planner_phases::main::inputs::runtime_dialect::runtime_dialect;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::{PyDict, PySet, PySetMethods};
use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::boundary::models::KeywordArguments;
use sqlbuild_python_base::native_host::types::Syntax;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::main::fields::mapping_get::mapping_get;
use sqlbuild_python_project::compiled_views::main::compiled_models::destination::destination;
use sqlbuild_python_project::compiled_views::main::compiled_models::name_object::name_object;

/// Decide column renames and return the existing planning result model.
#[pyfunction(name = "plan_column_migrations")]
#[pyo3(signature = (**options))]
pub(crate) fn plan_column_migrations_of<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<()> {
    let args = KeywordArguments::read(
        py,
        "plan_column_migrations",
        options,
        (
            &[
                "runtime",
                "scope",
                "snapshot",
                "full_refresh_model_names",
                "physical_relations",
                "overrides",
                "deferral",
                "source_columns",
            ],
            &[],
        ),
    )?;
    plan_column_migrations(
        &MigrationPhase {
            runtime: args.value("runtime")?,
            scope: args.value("scope")?,
            snapshot: args.value("snapshot")?,
            overrides: args.value("overrides")?,
            deferral: args.value("deferral")?,
        },
        &ColumnMigrationInputs {
            full_refresh_model_names: args.value("full_refresh_model_names")?,
            physical_relations: args.value("physical_relations")?,
            source_columns: args.value("source_columns")?,
        },
    )
    .map(|_| ())
}

/// Decide column renames for the selected incremental and snapshot models.
pub(crate) fn plan_column_migrations<'py>(
    phase: &MigrationPhase<'py>,
    columns: &ColumnMigrationInputs<'py>,
) -> PyResult<ColumnMigrationPlanning<'py>> {
    let py = phase.runtime.py();
    let runtime = phase.runtime.clone();
    let snapshot = phase.snapshot.clone();
    let dialect = runtime_dialect(&runtime)?;
    let candidates = super::column_candidates::candidates(
        (phase, columns),
        &dialect.syntax("sql_analysis_dialect")?.call0()?,
    )?;
    if candidates.is_empty() {
        return Ok(ColumnMigrationPlanning {
            snapshot,
            entries: Vec::new(),
            hints: Vec::new(),
            warnings: Vec::new(),
        });
    }
    let state = crate::planner_migrations::_helpers::discovery::state_inspection::inspection(
        &runtime, None,
    )?;
    let schemas = PySet::empty(py)?;
    for candidate in &candidates {
        let schema = native_field(&destination(&candidate.model)?, "schema")?;
        if !schema.is_none() {
            schemas.add(schema)?;
        }
    }
    call_with(
        &native_field(&state, "inspect_column_events")?,
        &[("schemas", schemas.into_any())],
    )?;
    let inputs = Bound::new(
        py,
        KnownInputColumns::new(
            &native_field(&snapshot, "existing_columns")?,
            &columns.source_columns,
        ),
    )?
    .into_any();
    let mut entries: Vec<Bound<'py, PyAny>> = Vec::new();
    let mut warnings: Vec<Bound<'py, PyAny>> = Vec::new();
    let mut hints: Vec<Bound<'py, PyAny>> = Vec::new();
    for candidate in &candidates {
        let decided = super::column_entries::decide(
            &runtime,
            candidate,
            &native_field(&state, "column_events")?,
            &inputs,
        )?;
        let physical = mapping_get(&columns.physical_relations, name_object(&candidate.model)?)?;
        let checked = super::column_entries::checked(&runtime, candidate, decided, &physical)?;
        for hint in super::column_entries::hints(candidate, &checked.entries)?.try_iter()? {
            hints.push(hint?);
        }
        entries.extend(checked.entries);
        warnings.extend(checked.warnings);
    }
    let policy =
        crate::planner_migrations::_helpers::planning::planning_boundary::missing_origin_policy(
            &runtime,
        )?;
    for entry in &mut entries {
        *entry = replace(entry, &[("missing_origin_policy", policy.clone())])?;
        if let Some(warning) = super::column_warnings::entry_warning(entry)? {
            warnings.push(warning);
        }
    }
    let overlaid = super::column_overlay::overlay(&snapshot, &candidates, &entries, &inputs)?;
    let snapshot = super::column_overlay::cursors((phase, columns), overlaid, &entries)?;
    Ok(ColumnMigrationPlanning {
        snapshot,
        entries,
        hints,
        warnings,
    })
}
