//! Convert existing planner contracts for origin compatibility checks.

use crate::planner_changes::main::detect_model_changes::detect_model_changes;
use crate::planner_migrations::models::{CompatibilityCheck, MigrationCompatibilityResult};
use pyo3::IntoPyObject;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods, PyTuple, PyTupleMethods};
use sqlbuild_analysis::planner_migrations::main::compatibility_findings::compatibility_findings;
use sqlbuild_analysis::planner_migrations::models::CompatibilityFinding;
use sqlbuild_python_base::boundary::main::keywords::keywords;
use sqlbuild_python_base::records::main::construct::enum_from_value::enum_from_value;
use sqlbuild_python_base::records::main::construct::new_record::new_record;
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::RecordId;
use sqlbuild_python_front::model_config::main::config::config_value::config_value;
use sqlbuild_python_front::model_config::main::config::model_values::model_values;
use sqlbuild_python_project::compiled_views::main::compiled_models::inferred_rows::inferred_rows;
use sqlbuild_python_project::compiled_views::main::compiled_models::name_object::name_object;

fn config(model: &Bound<'_, PyAny>, key: &str) -> PyResult<Option<String>> {
    config_value(model.py(), &model_values(model)?, key)?.extract()
}

/// The schema findings that block migrating one model's history; `snapshots_config` is given for snapshots.
fn blocking<'py>(
    (model, findings_in): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
    dialect: &Bound<'py, PyAny>,
    snapshots_config: Option<&Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyTuple>> {
    let py = model.py();
    let snapshot = snapshots_config.is_some();
    let policy = if let Some(snapshots_config) = snapshots_config {
        config(model, "snapshot_schema_change")?
            .filter(|value| !value.is_empty())
            .unwrap_or(native_field(snapshots_config, "schema_change")?.extract()?)
    } else {
        config(model, "on_schema_change")?
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "append_new_columns".to_owned())
    };
    new_record(
        py,
        if snapshot {
            RecordId::SnapshotSchemaChangePolicy
        } else {
            RecordId::OnSchemaChange
        },
        (&policy,),
        None,
    )?;
    let mut findings: Vec<CompatibilityFinding> = Vec::new();
    for finding in findings_in.try_iter()? {
        let finding = finding?;
        findings.push(CompatibilityFinding {
            kind: native_field(&finding, "kind")?.extract()?,
            column_name: native_field(&finding, "column_name")?.extract()?,
            expected_type: native_field(&finding, "expected_type")?.extract()?,
            actual_type: native_field(&finding, "actual_type")?.extract()?,
        });
    }
    let dialect: Option<String> = dialect.extract()?;
    PyTuple::new(
        py,
        compatibility_findings(
            &findings,
            (&policy, snapshot),
            dialect.as_deref().unwrap_or("generic"),
        )
        .map_err(PyValueError::new_err)?,
    )
}
/// Whether an origin relation's columns can carry the destination model's history.
pub(crate) fn check_migration_compatibility<'py>(
    check: &CompatibilityCheck<'py>,
) -> PyResult<MigrationCompatibilityResult<'py>> {
    let py = check.model.py();
    let model = check.model.clone();
    let relation = check.origin_relation.clone();
    let relation_type: String = native_field(&relation, "relation_type")?.extract()?;
    if relation_type.to_uppercase().contains("VIEW") {
        return result(
            py,
            "incompatible",
            PyTuple::new(
                py,
                ["origin relation is a view; only tables carry migratable history"],
            )?,
        );
    }
    let relations = PyDict::new(py);
    relations.set_item(name_object(&model)?, relation)?;
    let columns = PyDict::new(py);
    columns.set_item(name_object(&model)?, check.origin_columns.clone())?;
    let snapshot = new_record_with(
        py,
        RecordId::WarehouseSnapshot,
        &[
            ("existing_relations", relations.into_any()),
            ("existing_columns", columns.into_any()),
            ("column_dialect", check.column_dialect.clone()),
        ],
    )?;
    let change = detect_model_changes(
        py,
        Some(keywords(
            py,
            &[
                ("model", model.clone()),
                ("snapshot", snapshot),
                ("sql_analysis_enabled", check.sql_analysis_enabled.clone()),
                (
                    "query_change_tracking",
                    false.into_pyobject(py)?.to_owned().into_any(),
                ),
                (
                    "full_refresh",
                    false.into_pyobject(py)?.to_owned().into_any(),
                ),
            ],
        )?),
    )?;
    let schema = native_field(&model, "schema_entry")?;
    let mut has_columns = inferred_rows(&model)?.is_some_and(|rows| !rows.is_empty());
    if !schema.is_none() {
        for column in native_field(&schema, "columns")?.try_iter()? {
            has_columns |= !native_field(&column?, "type")?.is_none();
        }
    }
    if !has_columns {
        return result(py, "not_checked", PyTuple::empty(py));
    }
    let snapshot = config(&model, "materialized")?.as_deref() == Some("snapshot");
    let snapshots_config = check.snapshots_config.clone();
    let findings = blocking(
        (&model, &native_field(&change, "schema_findings")?),
        &check.column_dialect,
        snapshot.then_some(&snapshots_config),
    )?;
    result(
        py,
        if findings.is_empty() {
            "compatible"
        } else {
            "incompatible"
        },
        findings,
    )
}

fn result<'py>(
    py: Python<'py>,
    status: &str,
    findings: Bound<'py, PyTuple>,
) -> PyResult<MigrationCompatibilityResult<'py>> {
    Ok(MigrationCompatibilityResult {
        status: enum_from_value(py, RecordId::MigrationCompatibility, status)?,
        findings: findings.into_any(),
    })
}
