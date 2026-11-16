//! Project inherited fingerprints and live relation facts before normal planning.

use crate::planner_migrations::_helpers::column::column_objects::native;
use crate::planner_migrations::_helpers::column::column_objects::{config, replace};
use crate::planner_migrations::models::MigrationPhase;
use crate::planner_phases::main::inputs::runtime_field::runtime_field;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use pyo3::types::{PyDict, PyDictMethods};
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::RecordId;

pub(crate) fn overlay<'py>(
    phase: &MigrationPhase<'py>,
    state: &Bound<'py, PyAny>,
    entries: &[Bound<'py, PyAny>],
    handovers: &Bound<'py, PyDict>,
) -> PyResult<Bound<'py, PyAny>> {
    let snapshot = phase.snapshot.clone();
    if handovers.is_empty() {
        return Ok(snapshot);
    }
    let runtime = phase.runtime.clone();
    let py = runtime.py();
    let relations = native_field(&snapshot, "existing_relations")?
        .cast_into::<PyDict>()?
        .copy()?;
    let columns = native_field(&snapshot, "existing_columns")?
        .cast_into::<PyDict>()?
        .copy()?;
    let fingerprints = native_field(&snapshot, "fingerprints")?;
    let models = native_field(&fingerprints, "models")?
        .cast_into::<PyDict>()?
        .copy()?;
    let redirected = PyDict::new(py);
    for entry in entries {
        let name = native_field(entry, "model_name")?;
        let Some(handover) = handovers.get_item(&name)? else {
            continue;
        };
        if handover.is_none() {
            models.call_method1("pop", (&name, py.None()))?;
        } else {
            models.set_item(&name, handover)?;
        }
        if !native_field(&native_field(entry, "decision")?, "moves_data")?.is_truthy()? {
            continue;
        }
        let origin = native_field(entry, "origin")?;
        let origin_relation = state.call_method1("relation", (&origin,))?;
        if origin_relation.is_none() {
            continue;
        }
        let destination = native_field(entry, "destination")?;
        let transient = native_field(entry, "stage_is_transient")?;
        relations.set_item(
            &name,
            replace(
                &origin_relation,
                &[
                    ("database", native_field(&destination, "database")?),
                    ("schema", native_field(&destination, "schema")?),
                    ("name", native_field(&destination, "name")?),
                    (
                        "is_transient",
                        if transient.is_none() {
                            native_field(&origin_relation, "is_transient")?
                        } else {
                            transient
                        },
                    ),
                ],
            )?,
        )?;
        columns.set_item(&name, state.call_method1("relation_columns", (&origin,))?)?;
        let qualified = native_field(&origin, "qualified_name")?;
        if !qualified.is_none() && cursor_enabled(&runtime, &name)? {
            redirected.set_item(name, qualified)?;
        }
    }
    let cursors = native_field(&snapshot, "cursor_snapshots")?
        .cast_into::<PyDict>()?
        .copy()?;
    let gathered = crate::planner_migrations::_helpers::planning::planning_boundary::redirected_cursor_snapshots(
        phase,
        (relations.clone().into_any(), redirected.into_any()),
        None,
    )?;
    cursors.call_method1("update", (gathered,))?;
    replace(
        &snapshot,
        &[
            ("existing_relations", relations.into_any()),
            ("existing_columns", columns.into_any()),
            (
                "fingerprints",
                native(
                    py,
                    RecordId::WarehouseFingerprints,
                    &[
                        ("models", models.into_any()),
                        ("functions", native_field(&fingerprints, "functions")?),
                        ("seeds", native_field(&fingerprints, "seeds")?),
                        ("python_nodes", native_field(&fingerprints, "python_nodes")?),
                    ],
                )?,
            ),
            ("cursor_snapshots", cursors.into_any()),
        ],
    )
}

fn cursor_enabled(runtime: &Bound<'_, PyAny>, name: &Bound<'_, PyAny>) -> PyResult<bool> {
    for item in native_field(
        &runtime_field(runtime, |runtime| &runtime.project)?,
        "models",
    )?
    .try_iter()?
    {
        let item = item?;
        if native_field(&item, "name")?.eq(name)? {
            return Ok(config(&item, "materialized")?.eq("incremental")?
                && !config(&item, "cursor")?.is_none());
        }
    }
    Ok(false)
}
