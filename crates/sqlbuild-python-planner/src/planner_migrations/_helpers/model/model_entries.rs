//! Assemble model migration decisions from inspected warehouse facts.

use sqlbuild_host::entries::models::ExecutorHost;

use crate::planner_migrations::_helpers::column::column_objects::lower;
use crate::planner_migrations::_helpers::column::column_objects::model;
use crate::planner_migrations::_helpers::column::column_objects::replace;
use crate::planner_migrations::_helpers::column::column_objects::text;
use crate::planner_migrations::_helpers::model::model_handover::applies;
use crate::planner_migrations::_helpers::model::model_handover::empty_version;
use crate::planner_migrations::_helpers::model::model_handover::handover;
use crate::planner_migrations::_helpers::planning::compatibility::check_migration_compatibility;
use crate::planner_migrations::models::CompatibilityCheck;
use crate::planner_migrations::models::MigrationDecisionRequest;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use pyo3::types::PyBool;
use sqlbuild_analysis::planner_migrations::main::model_decision::model_decision;
use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::native_host::main::host_entry::host_entry;
use sqlbuild_python_base::records::main::construct::enum_from_value::enum_from_value;
use sqlbuild_python_base::records::main::construct::enum_member::enum_member;
use sqlbuild_python_base::records::main::construct::error_class::error_class;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::ErrorId;
use sqlbuild_python_base::records::types::RecordId;
use sqlbuild_python_front::state_rows::main::migrations::newest_move::newest_move;
use sqlbuild_python_project::compiled_views::main::compiled_models::name_object::name_object;

pub(crate) fn relation<'py>(location: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    host_entry(location.py(), ExecutorHost::MigrationRelationForLocation)?.call1((location,))
}

fn key(location: &Bound<'_, PyAny>) -> PyResult<(String, String)> {
    let schema = native_field(location, "schema")?;
    Ok((
        if schema.is_truthy()? {
            lower(&schema)?
        } else {
            String::new()
        },
        lower(&native_field(location, "name")?)?,
    ))
}

pub(crate) fn decision<'py>(entry: &Bound<'py, PyAny>, name: &str) -> PyResult<Bound<'py, PyAny>> {
    replace(
        entry,
        &[(
            "decision",
            enum_from_value(entry.py(), RecordId::MigrationDecision, name)?,
        )],
    )
}

fn result<'py>(
    entry: Bound<'py, PyAny>,
    inherited: Bound<'py, PyAny>,
    applies: bool,
) -> PyResult<MigrationDecision<'py>> {
    Ok(MigrationDecision {
        entry,
        inherited,
        applies,
    })
}

/// The decided plan entry, the inherited fingerprint, and whether the history handover applies.
pub(crate) struct MigrationDecision<'py> {
    pub(super) entry: Bound<'py, PyAny>,
    pub(super) inherited: Bound<'py, PyAny>,
    pub(super) applies: bool,
}

pub(crate) fn decide<'py>(
    decision_request: &MigrationDecisionRequest<'_, 'py>,
) -> PyResult<MigrationDecision<'py>> {
    let runtime = decision_request.runtime.clone();
    let py = runtime.py();
    let request = decision_request.request;
    let item = request.model.clone();
    let destination =
        sqlbuild_python_project::compiled_views::main::compiled_models::destination::destination(
            &item,
        )?;
    let origin = decision_request.origin.clone();
    let snapshot = decision_request.snapshot.clone();
    let state = decision_request.state.clone();
    let origin_model = decision_request.origin_model.clone();
    if key(&origin)? == key(&destination)? {
        let name: String = native_field(&item, "name")?.extract()?;
        return Err(pyo3::PyErr::from_value(
            error_class(py, ErrorId::PlannerInput)?.call1((format!(
                "model '{name}': migrate_from names the model's own relation"
            ),))?,
        ));
    }
    let newest = newest_move(&native_field(&state, "events")?, &relation(&destination)?)?;
    let origin_relation = state.call_method1("relation", (&origin,))?;
    let fingerprint = call_with(
        &native_field(&state, "model_fingerprint")?,
        &[
            ("location", origin.clone()),
            ("model_name", origin_model.clone()),
        ],
    )?;
    let base = model(
        py,
        RecordId::ModelMigrationPlanEntry,
        &[
            ("model_name", name_object(&item)?),
            ("discovery", request.discovery.clone()),
            (
                "decision",
                enum_member(py, RecordId::MigrationDecision, "MIGRATE")?,
            ),
            (
                "compatibility",
                enum_member(py, RecordId::MigrationCompatibility, "NOT_CHECKED")?,
            ),
            ("origin_model", origin_model),
            ("origin", origin.clone()),
            ("destination", destination.clone()),
            (
                "target_name",
                native_field(&native_field(&runtime, "project")?, "effective_target_name")?,
            ),
            (
                "origin_version_hash",
                if fingerprint.is_none() {
                    text(py, "")
                } else {
                    native_field(&fingerprint, "version_hash")?
                },
            ),
            (
                "origin_is_transient",
                PyBool::new(
                    py,
                    !origin_relation.is_none()
                        && native_field(&origin_relation, "is_transient")?.is_truthy()?,
                )
                .to_owned()
                .into_any(),
            ),
            (
                "origin_tracked",
                PyBool::new(py, !fingerprint.is_none())
                    .to_owned()
                    .into_any(),
            ),
        ],
    )?;
    let recorded = !newest.is_none()
        && native_field(&newest, "destination")?
            .call_method1("matches", (relation(&destination)?,))?
            .is_truthy()?
        && native_field(&newest, "origin")?
            .call_method1("matches", (relation(&origin)?,))?
            .is_truthy()?;
    if request.identity_only {
        let recorded = recorded && native_field(&newest, "decision")?.eq("renamed")?;
        if request.discovery.eq("manual")? && !recorded && origin_relation.is_none() {
            return result(
                decision(&base, "origin_missing")?,
                py.None().into_bound(py),
                false,
            );
        }
        let inherited = if recorded && !applies(&item, &newest, &snapshot)? {
            native_field(&native_field(&snapshot, "fingerprints")?, "models")?
                .call_method1("get", (name_object(&item)?,))?
        } else {
            empty_version(&item, &fingerprint)?
        };
        return result(
            replace(
                &decision(&base, "renamed")?,
                &[(
                    "completed_at",
                    if recorded {
                        native_field(&newest, "created_at")?
                    } else {
                        py.None().into_bound(py)
                    },
                )],
            )?,
            inherited,
            true,
        );
    }
    if recorded {
        let applies = applies(&item, &newest, &snapshot)?;
        let inherited = if applies {
            handover(
                &item,
                &fingerprint,
                &native_field(&newest, "origin_version_hash")?,
            )?
        } else {
            native_field(&native_field(&snapshot, "fingerprints")?, "models")?
                .call_method1("get", (native_field(&item, "name")?,))?
        };
        return result(
            replace(
                &decision(&base, "done")?,
                &[
                    ("completed_at", native_field(&newest, "created_at")?),
                    (
                        "origin_version_hash",
                        native_field(&newest, "origin_version_hash")?,
                    ),
                    ("target_name", native_field(&newest, "target_name")?),
                ],
            )?,
            inherited,
            applies,
        );
    }
    if origin_relation.is_none() {
        return result(
            decision(&base, "origin_missing")?,
            py.None().into_bound(py),
            false,
        );
    }
    let compatibility = check_migration_compatibility(&CompatibilityCheck {
        model: item.clone(),
        origin_relation,
        origin_columns: state.call_method1("relation_columns", (&origin,))?,
        sql_analysis_enabled: native_field(
            &native_field(&native_field(&runtime, "project")?, "settings")?,
            "sql_analysis",
        )?,
        column_dialect: native_field(&snapshot, "column_dialect")?,
        snapshots_config: decision_request.snapshots_config.clone(),
    })?;
    let name = model_decision(
        native_field(&snapshot, "existing_relations")?.contains(native_field(&item, "name")?)?,
        (
            !newest.is_none(),
            !newest.is_none()
                && native_field(&newest, "origin")?
                    .call_method1("matches", (relation(&destination)?,))?
                    .is_truthy()?,
            native_field(&native_field(&snapshot, "fingerprints")?, "models")?
                .contains(native_field(&item, "name")?)?,
        ),
        request.force,
    );
    let entry = replace(
        &decision(&base, name)?,
        &[
            ("compatibility", compatibility.status),
            ("compatibility_findings", compatibility.findings),
        ],
    )?;
    if !native_field(&native_field(&entry, "decision")?, "moves_data")?.is_truthy()?
        || native_field(&entry, "blocks_build")?.is_truthy()?
    {
        return result(entry, py.None().into_bound(py), false);
    }
    let inherited = handover(
        &item,
        &fingerprint,
        &native_field(&entry, "origin_version_hash")?,
    )?;
    result(
        crate::planner_migrations::_helpers::planning::planning_execution::migration_with_execution(&runtime, &item, &entry)?,
        inherited,
        true,
    )
}
