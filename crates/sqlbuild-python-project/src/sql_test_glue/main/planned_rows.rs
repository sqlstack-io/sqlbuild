//! Native plans of one SQL-test batch, with adapter rendering and timing accounted for.

use crate::sql_test_glue::_helpers::plan::planning::{PlanningRequest, plan_compiled_sql_tests};
use crate::sql_test_glue::models::PlanRow;
use crate::sql_test_glue::models::SqlTestPlanOptions;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_host::compile_context::main::add_compile_timing::add_compile_timing;
use sqlbuild_python_base::boundary::main::imported::imported;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// Native plans of one SQL-test batch, with adapter rendering and timing accounted for.
pub fn planned_rows<'py>(
    (project, adapter): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
    tests: &Bound<'py, PyAny>,
    options: &SqlTestPlanOptions<'py>,
) -> PyResult<Vec<PlanRow>> {
    let py = project.py();
    let artifact = options.artifact;
    if !tests.is_truthy()? {
        return Ok(Vec::new());
    }
    let start: u64 = imported(py, "time", "perf_counter_ns")?
        .call0()?
        .extract()?;
    let model_sql = crate::sql_test_glue::_helpers::plan::plan_rendering::models(project, adapter)?;
    let functions =
        crate::sql_test_glue::_helpers::plan::plan_rendering::functions(project, adapter)?;
    let overrides =
        crate::sql_test_glue::_helpers::plan::plan_rendering::overrides(project, tests, adapter)?;
    let request = PlanningRequest {
        models: native_field(project, "models")?.extract()?,
        rendered_model_sql: model_sql.extract()?,
        functions: functions.extract()?,
        tests: tests.extract()?,
        rendered_test_overrides: overrides.extract()?,
        sql_analysis_enabled: options.sql_analysis_enabled.extract()?,
        sql_analysis_dialect: adapter.call_method0("sql_analysis_dialect")?.extract()?,
        set_difference_operator: adapter
            .call_method0("render_set_difference_operator")?
            .extract()?,
        requires_derived_table_aliases: adapter
            .call_method0("requires_derived_table_aliases")?
            .extract()?,
        lexical_syntax: native_field(
            &native_field(project, "sql_lexical_syntax")?,
            "native_mapping",
        )?
        .extract()?,
        render_sql: artifact || options.render_sql,
        include_plan: !artifact && options.include_plan,
        max_unit_test_real_models: native_field(
            &native_field(project, "settings")?,
            "unit_test_max_real_models",
        )?
        .extract()?,
    };
    let (rows, planning_ns, rendering_ns) = match plan_compiled_sql_tests(py, request) {
        Ok(value) => value,
        Err(error) if error.is_instance_of::<pyo3::exceptions::PyValueError>(py) => {
            return Err(
                crate::sql_test_glue::_helpers::plan::plan_errors::planning_error(
                    py, error, tests,
                )?,
            );
        }
        Err(error) => return Err(error),
    };
    let end: u64 = imported(py, "time", "perf_counter_ns")?
        .call0()?
        .extract()?;
    for (phase, elapsed) in [
        (
            "test_planning_ms",
            planning_ns
                + end
                    .saturating_sub(start)
                    .saturating_sub(planning_ns)
                    .saturating_sub(rendering_ns),
        ),
        ("comparison_render_ms", rendering_ns),
    ] {
        add_compile_timing(phase, elapsed);
    }
    if artifact {
        for row in &rows {
            row.rendered_sql(py)?;
        }
    }
    Ok(rows)
}
