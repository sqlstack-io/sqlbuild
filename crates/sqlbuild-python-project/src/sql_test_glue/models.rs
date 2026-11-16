//! Public records of sql test glue.

use pyo3::prelude::{Bound, PyAny};
use sqlbuild_analysis::compiler::models::SqlTestPlan;

/// How one batch of SQL tests is planned.
#[derive(Debug)]
pub struct SqlTestPlanOptions<'py> {
    pub sql_analysis_enabled: Bound<'py, PyAny>,
    pub render_sql: bool,
    pub include_plan: bool,
    pub artifact: bool,
}

/// How assembling SQL-test plan results validates fixtures.
#[derive(Debug)]
pub struct SqlTestAssembly<'py> {
    pub sql_analysis_enabled: Bound<'py, PyAny>,
    pub validate_fixtures: bool,
    pub fixture_planning_context: Bound<'py, PyAny>,
}

/// A native SQL-test plan with its distinct error messages.
#[derive(Debug)]
pub struct PlanRow {
    pub plan: SqlTestPlan,
    pub error_messages: Vec<String>,
}
