//! The planner's Python change-detection result models.

use crate::planner_changes::main::native_enum::native_enum;
use crate::planner_changes::main::planner_enum::planner_enum;
use pyo3::prelude::{Bound, PyAny, PyResult, Python};
use pyo3::types::PyTuple;
use pyo3::{IntoPyObject, PyErr};
use sqlbuild_python_base::records::main::construct::new_plain_error::new_plain_error;

use sqlbuild_analysis::planner_changes::errors::UnknownReplayPolicy;
use sqlbuild_analysis::planner_changes::models::{
    Backfill, BackfillAction, ChangeKind, SchemaChangeKind, SchemaColumnSource, SchemaFindingRow,
};

use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use sqlbuild_python_base::records::types::ErrorId;
use sqlbuild_python_base::records::types::RecordId;

/// `PlannerInputError(message)` for an unknown replay policy.
pub(crate) fn replay_policy_error(py: Python<'_>, error: UnknownReplayPolicy) -> PyErr {
    new_plain_error(py, ErrorId::PlannerInput, &error.message())
}

/// `BackfillResult(action=..., duration=...)`.
pub(crate) fn backfill_result<'py>(
    py: Python<'py>,
    backfill: &Backfill,
) -> PyResult<Bound<'py, PyAny>> {
    let action: &str = match backfill.action {
        BackfillAction::Full => "full",
        BackfillAction::Bounded => "bounded",
        BackfillAction::ForwardOnly => "forward",
    };
    new_record_with(
        py,
        RecordId::BackfillResult,
        &[
            (
                "action",
                planner_enum(py, RecordId::BackfillAction, action)?,
            ),
            (
                "duration",
                backfill.duration.clone().into_pyobject(py)?.into_any(),
            ),
        ],
    )
}

/// The `ChangeKind` member of a decided change.
pub(crate) fn change_kind<'py>(py: Python<'py>, kind: ChangeKind) -> PyResult<Bound<'py, PyAny>> {
    let value: &str = match kind {
        ChangeKind::QueryChanged => "query_changed",
        ChangeKind::FunctionChanged => "function_changed",
        ChangeKind::SchemaChanged => "schema_changed",
        ChangeKind::ConfigChanged => "config_changed",
        ChangeKind::NoChange => "no_change",
    };
    native_enum(py, RecordId::ChangeKind, value)
}

/// A tuple of `SchemaFinding` values.
pub(crate) fn schema_findings<'py>(
    py: Python<'py>,
    findings: &[SchemaFindingRow],
) -> PyResult<Bound<'py, PyTuple>> {
    let mut items: Vec<Bound<'py, PyAny>> = Vec::with_capacity(findings.len());
    for finding in findings {
        let kind: &str = match finding.kind {
            SchemaChangeKind::ColumnAdded => "column_added",
            SchemaChangeKind::ColumnRemoved => "column_removed",
            SchemaChangeKind::ColumnTypeChanged => "column_type_changed",
        };
        let source: &str = match finding.source {
            SchemaColumnSource::Yml => "yml",
            SchemaColumnSource::SqlAnalysis => "sql_analysis",
        };
        items.push(new_record_with(
            py,
            RecordId::SchemaFinding,
            &[
                ("kind", native_enum(py, RecordId::SchemaChangeKind, kind)?),
                (
                    "column_name",
                    finding.column_name.clone().into_pyobject(py)?.into_any(),
                ),
                (
                    "source",
                    native_enum(py, RecordId::SchemaColumnSource, source)?,
                ),
                (
                    "expected_type",
                    finding.expected_type.clone().into_pyobject(py)?.into_any(),
                ),
                (
                    "actual_type",
                    finding.actual_type.clone().into_pyobject(py)?.into_any(),
                ),
            ],
        )?);
    }
    PyTuple::new(py, items)
}
