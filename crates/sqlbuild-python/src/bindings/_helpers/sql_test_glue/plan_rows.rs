//! Materialize native plan rows as the existing executor-facing dataclasses.

use crate::bindings::_helpers::sql_test_glue::planning::PlanRow;
use pyo3::IntoPyObjectExt;
use pyo3::prelude::{Bound, PyAny, PyResult, Python};
use pyo3::types::PyTuple;
use sqlbuild_analysis::compiler::models::{
    SqlTestAssertionStep, SqlTestChainStep, SqlTestPlan, SqlTestPlanWarning,
};
use sqlbuild_python_base::records::main::construct::enum_from_value::enum_from_value;
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use sqlbuild_python_base::records::types::RecordId;

fn chain_step<'py>(py: Python<'py>, step: SqlTestChainStep) -> PyResult<Bound<'py, PyAny>> {
    let SqlTestChainStep {
        model_name: model,
        resolved_sql: sql,
        expected_cte_sql: expected_sql,
        expected_lifted_ctes: expected_lifted,
        lifted_ctes: lifted,
        comparison_body_sql: body,
        expected_columns,
    } = step;
    new_record_with(
        py,
        RecordId::ChainStep,
        &[
            ("model_name", model.into_bound_py_any(py)?),
            ("resolved_sql", sql.into_bound_py_any(py)?),
            ("expected_cte_sql", expected_sql.into_bound_py_any(py)?),
            ("lifted_ctes", PyTuple::new(py, lifted)?.into_any()),
            ("comparison_body_sql", body.into_bound_py_any(py)?),
            (
                "expected_columns",
                match expected_columns {
                    Some(columns) => PyTuple::new(py, columns)?.into_any(),
                    None => py.None().into_bound(py),
                },
            ),
            (
                "expected_lifted_ctes",
                PyTuple::new(py, expected_lifted)?.into_any(),
            ),
        ],
    )
}

fn assertion_step<'py>(py: Python<'py>, step: SqlTestAssertionStep) -> PyResult<Bound<'py, PyAny>> {
    let SqlTestAssertionStep {
        name,
        resolved_sql: sql,
        lifted_ctes: lifted,
        comparison_body_sql: body,
    } = step;
    new_record_with(
        py,
        RecordId::SqlTestAssertionStep,
        &[
            ("name", name.into_bound_py_any(py)?),
            ("resolved_sql", sql.into_bound_py_any(py)?),
            ("lifted_ctes", PyTuple::new(py, lifted)?.into_any()),
            ("comparison_body_sql", body.into_bound_py_any(py)?),
        ],
    )
}

pub(super) fn plan(py: Python<'_>, row: PlanRow) -> PyResult<Bound<'_, PyAny>> {
    let PlanRow {
        plan:
            SqlTestPlan {
                sql,
                chain: chain_rows,
                assertions: assertion_rows,
                model_names,
                warnings: warning_rows,
            },
        error_messages,
    } = row;
    let mut chain: Vec<Bound<'_, PyAny>> = Vec::new();
    for value in chain_rows {
        chain.push(chain_step(py, value)?);
    }
    let mut assertions: Vec<Bound<'_, PyAny>> = Vec::new();
    for value in assertion_rows {
        assertions.push(assertion_step(py, value)?);
    }
    let mut warnings: Vec<Bound<'_, PyAny>> = Vec::new();
    for SqlTestPlanWarning {
        model_name,
        severity: level,
        message,
    } in warning_rows
    {
        warnings.push(new_record_with(
            py,
            RecordId::PlanWarning,
            &[
                ("model_name", model_name.into_bound_py_any(py)?),
                (
                    "severity",
                    enum_from_value(py, RecordId::WarningSeverity, level)?,
                ),
                ("message", message.into_bound_py_any(py)?),
            ],
        )?);
    }
    new_record_with(
        py,
        RecordId::NativeSqlTestPlan,
        &[
            ("chain", PyTuple::new(py, chain)?.into_any()),
            ("assertions", PyTuple::new(py, assertions)?.into_any()),
            ("model_names", PyTuple::new(py, model_names)?.into_any()),
            ("warnings", PyTuple::new(py, warnings)?.into_any()),
            (
                "error_messages",
                PyTuple::new(py, error_messages)?.into_any(),
            ),
            ("sql", sql.into_bound_py_any(py)?),
        ],
    )
}
