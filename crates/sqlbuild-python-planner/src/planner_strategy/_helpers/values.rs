//! Conversions between the planner's Python models and the native strategy rows.

use crate::planner_changes::main::native_enum::native_enum;
use crate::planner_strategy::main::values::text::text;
use pyo3::IntoPyObject;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::PyString;
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use sqlbuild_python_front::model_config::main::config::config_value::config_value as typed_config_value;
use sqlbuild_python_front::model_config::main::config::model_values::model_values;

use sqlbuild_analysis::planner_changes::models::{
    SchemaChangeKind, SchemaColumnSource, SchemaFindingRow,
};
use sqlbuild_analysis::planner_strategy::main::materialization_type::materialization_type;
use sqlbuild_analysis::planner_strategy::models::{
    ChangeFacts, MaterializationType, PlanChangeKind, WarningRow, WarningSeverity,
};

use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::RecordId;

const YML_SOURCE: &str = "yml";
const FULL_BACKFILL: &str = "full";
const FORWARD_ONLY_BACKFILL: &str = "forward";

/// A string value, or `None` for `None` and anything that is not a string.
pub(crate) fn optional_string(value: &Bound<'_, PyAny>) -> PyResult<Option<String>> {
    if value.is_instance_of::<PyString>() {
        return text(value).map(Some);
    }
    Ok(None)
}

/// `values.get(key)` of a model's config.
pub(crate) fn config_value<'py>(
    model: &Bound<'py, PyAny>,
    key: &str,
) -> PyResult<Bound<'py, PyAny>> {
    typed_config_value(model.py(), &model_values(model)?, key)
}

/// The materialization type of a model's `materialized` config.
pub(crate) fn model_materialization(model: &Bound<'_, PyAny>) -> PyResult<MaterializationType> {
    let materialized: Option<String> = optional_string(&config_value(model, "materialized")?)?;
    Ok(materialization_type(materialized.as_deref()))
}

/// The `MaterializationType` value of a materialization.
pub(crate) fn materialization_value(materialization: MaterializationType) -> &'static str {
    match materialization {
        MaterializationType::View => "view",
        MaterializationType::Table => "table",
        MaterializationType::Incremental => "incremental",
        MaterializationType::Snapshot => "snapshot",
        MaterializationType::Seed => "seed",
        MaterializationType::Custom => "custom",
        MaterializationType::Maintained => "maintained",
    }
}

fn change_kind(value: &str) -> PlanChangeKind {
    match value {
        "first_run" => PlanChangeKind::FirstRun,
        "renamed" => PlanChangeKind::Renamed,
        "query_changed" => PlanChangeKind::QueryChanged,
        "function_changed" => PlanChangeKind::FunctionChanged,
        "config_changed" => PlanChangeKind::ConfigChanged,
        "schema_changed" => PlanChangeKind::SchemaChanged,
        _ => PlanChangeKind::NoChange,
    }
}

/// One `SchemaFinding` as a native row.
fn finding_row(finding: &Bound<'_, PyAny>) -> PyResult<SchemaFindingRow> {
    let kind: SchemaChangeKind = match text(&native_field(finding, "kind")?)?.as_str() {
        "column_added" => SchemaChangeKind::ColumnAdded,
        "column_removed" => SchemaChangeKind::ColumnRemoved,
        _ => SchemaChangeKind::ColumnTypeChanged,
    };
    let source: SchemaColumnSource = if text(&native_field(finding, "source")?)? == YML_SOURCE {
        SchemaColumnSource::Yml
    } else {
        SchemaColumnSource::SqlAnalysis
    };
    Ok(SchemaFindingRow {
        kind,
        column_name: text(&native_field(finding, "column_name")?)?,
        source,
        expected_type: native_field(finding, "expected_type")?.extract()?,
        actual_type: native_field(finding, "actual_type")?.extract()?,
    })
}

/// A tuple of `SchemaFinding` values as native rows.
pub(crate) fn finding_rows(findings: &Bound<'_, PyAny>) -> PyResult<Vec<SchemaFindingRow>> {
    findings
        .try_iter()?
        .map(|finding| finding_row(&finding?))
        .collect()
}

/// The parts of a `ChangeDetectionResult` the strategy reads.
pub(crate) fn change_facts(change: &Bound<'_, PyAny>) -> PyResult<ChangeFacts> {
    let backfill: String = text(&native_field(&native_field(change, "backfill")?, "action")?)?;
    Ok(ChangeFacts {
        kind: change_kind(&text(&native_field(change, "change_kind")?)?),
        backfill_full: backfill == FULL_BACKFILL,
        backfill_forward_only: backfill == FORWARD_ONLY_BACKFILL,
        query_changed: native_field(change, "query_changed")?.is_truthy()?,
        changed_functions: native_field(change, "changed_functions")?
            .try_iter()?
            .map(|name| text(&name?))
            .collect::<PyResult<_>>()?,
        config_changed: native_field(change, "config_changed")?.is_truthy()?,
        schema_findings: finding_rows(&native_field(change, "schema_findings")?)?,
        recorded_build_relation_missing: native_field(change, "recorded_build_relation_missing")?
            .is_truthy()?,
    })
}

/// `PlanWarning(model_name=..., severity=..., message=..., code=...)`.
pub(crate) fn plan_warning<'py>(
    model_name: &Bound<'py, PyAny>,
    row: WarningRow,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = model_name.py();
    let severity: &str = match row.severity {
        WarningSeverity::Error => "error",
        WarningSeverity::Warning => "warning",
        WarningSeverity::Info => "info",
    };
    new_record_with(
        py,
        RecordId::PlanWarning,
        &[
            ("model_name", model_name.clone()),
            (
                "severity",
                native_enum(py, RecordId::WarningSeverity, severity)?,
            ),
            ("message", PyString::new(py, &row.message).into_any()),
            ("code", row.code.into_pyobject(py)?.into_any()),
        ],
    )
}
