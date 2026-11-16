//! Detect changes and build a plan entry and warnings for a single model.

use crate::planner_changes::main::detect_model_changes::detect_model_changes;
use crate::planner_entries::_helpers::entries::model_entries::ContextReads;
use crate::planner_entries::_helpers::entries::model_entries::ModelPlanRequest;
use crate::planner_entries::_helpers::entries::model_entries::plan_model_from_change;
use pyo3::prelude::{Bound, PyAny, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::PyDict;
use sqlbuild_python_base::boundary::main::keywords::keywords;
use sqlbuild_python_base::boundary::models::KeywordArguments;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// Detect changes and build a plan entry and warnings for a single model.
#[pyfunction]
#[pyo3(signature = (**kwargs))]
pub fn plan_model<'py>(
    py: Python<'py>,
    kwargs: Option<Bound<'py, PyDict>>,
) -> PyResult<(Bound<'py, PyAny>, Bound<'py, PyAny>)> {
    let args = KeywordArguments::read(
        py,
        "plan_model",
        kwargs,
        (
            &[
                "model",
                "snapshot",
                "adapter",
                "context",
                "sql_analysis_enabled",
                "query_change_tracking",
                "full_refresh",
                "cursor_overrides",
            ],
            &["external_sql_reference_resolver"],
        ),
    )?;
    let model = args.value("model")?;
    let snapshot = args.value("snapshot")?;
    let change_result = detect_model_changes(
        py,
        Some(keywords(
            py,
            &[
                ("model", model.clone()),
                ("snapshot", snapshot.clone()),
                ("sql_analysis_enabled", args.value("sql_analysis_enabled")?),
                (
                    "query_change_tracking",
                    args.value("query_change_tracking")?,
                ),
                ("full_refresh", args.value("full_refresh")?),
            ],
        )?),
    )?;
    let overrides = args.value("cursor_overrides")?;
    plan_model_from_change(&ModelPlanRequest {
        model,
        snapshot,
        context: &ContextReads::of(
            args.value("context")?,
            args.value("adapter")?,
            args.value("external_sql_reference_resolver")?,
        )?,
        sql_analysis_enabled: args.flag("sql_analysis_enabled")?,
        full_refresh: args.flag("full_refresh")?,
        overrides: (
            native_field(&overrides, "start_cursor_override")?,
            native_field(&overrides, "end_cursor_override")?,
        ),
        change_result,
        protect_full_refresh: true,
    })
}
