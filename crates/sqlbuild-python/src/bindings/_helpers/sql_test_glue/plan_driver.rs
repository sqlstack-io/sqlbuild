//! Host SQL-test request preparation, native planning, and Python result conversion.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyErr, PyResult, Python, pyfunction};
use pyo3::types::{PyDict, PyDictMethods, PyTuple};
use sqlbuild_host::compile_context::main::add_compile_timing::add_compile_timing;

use crate::bindings::_helpers::boundary::python_calls::{KeywordArguments, imported};
use crate::bindings::_helpers::planner_phases::objects::object;
use crate::bindings::_helpers::planner_strategy::fixture_context::count_chain_resolution;
use crate::bindings::_helpers::records::construct::{error_class, new_record_with};
use crate::bindings::_helpers::records::fields::field as native_field;
use crate::bindings::_helpers::records::ids::{ErrorId, RecordId};
use crate::bindings::_helpers::sql_test_glue::models::SqlTestPlanOptions;
use crate::bindings::_helpers::sql_test_glue::plan_values::tuple;
use crate::bindings::_helpers::sql_test_glue::planning::{
    ChainRequest, PlanRow, PlanningRequest, plan_compiled_sql_tests,
    resolve_compiled_sql_test_chains,
};

impl PlanRow {
    /// Rendered SQL for an artifact, or the planning error shared by native and Python consumers.
    pub(crate) fn rendered_sql(&self, py: Python<'_>) -> PyResult<&str> {
        let Some(sql) = self.plan.sql.as_deref() else {
            return Err(PyErr::from_value(
                error_class(py, ErrorId::NativeSqlTestPlanning)?
                    .call1(("native SQL-test planning omitted rendered SQL",))?,
            ));
        };
        Ok(sql)
    }
}

pub(super) fn chains<'py>(
    project: &Bound<'py, PyAny>,
    tests: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py = project.py();
    if !tests.is_truthy()? {
        return Ok(PyTuple::empty(py).into_any());
    }
    let request = ChainRequest {
        models: native_field(project, "models")?.extract()?,
        tests: tests.extract()?,
        lexical_syntax: native_field(
            &native_field(project, "sql_lexical_syntax")?,
            "native_mapping",
        )?
        .extract()?,
    };
    let native =
        resolve_compiled_sql_test_chains(py, request).and_then(|chains| object(py, chains));
    let native = match native {
        Ok(value) => value,
        Err(error) if error.is_instance_of::<pyo3::exceptions::PyValueError>(py) => {
            return Err(super::plan_errors::planning_error(py, error, tests)?);
        }
        Err(error) => return Err(error),
    };
    let mut result: Vec<Bound<'py, PyAny>> = Vec::new();
    for chain in native.try_iter()? {
        result.push(tuple(&chain?)?);
    }
    Ok(PyTuple::new(py, result)?.into_any())
}

/// The model chain of each SQL test, resolved in one project batch and counted.
pub(crate) fn resolve_sql_test_model_chains<'py>(
    project: &Bound<'py, PyAny>,
    tests: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    count_chain_resolution(project.py())?;
    chains(project, tests)
}

/// Native plans of one SQL-test batch, with adapter rendering and timing accounted for.
pub(crate) fn planned_rows<'py>(
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
    let model_sql = super::plan_rendering::models(project, adapter)?;
    let functions = super::plan_rendering::functions(project, adapter)?;
    let overrides = super::plan_rendering::overrides(project, tests, adapter)?;
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
            return Err(super::plan_errors::planning_error(py, error, tests)?);
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

/// Plans of one SQL-test batch, or their compiled artifacts, for Python consumers.
pub(crate) fn planned<'py>(
    (project, adapter): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
    tests: &Bound<'py, PyAny>,
    options: &SqlTestPlanOptions<'py>,
) -> PyResult<Bound<'py, PyTuple>> {
    let py = project.py();
    let rows = planned_rows((project, adapter), tests, options)?;
    let mut plans: Vec<Bound<'py, PyAny>> = Vec::new();
    for row in rows {
        let plan = super::plan_rows::plan(py, row)?;
        if options.artifact {
            let sql = native_field(&plan, "sql")?;
            plans.push(new_record_with(
                py,
                RecordId::NativeSqlTestArtifact,
                &[
                    ("sql", sql),
                    ("model_names", native_field(&plan, "model_names")?),
                    ("error_messages", native_field(&plan, "error_messages")?),
                ],
            )?);
        } else {
            plans.push(plan);
        }
    }
    PyTuple::new(py, plans)
}

#[pyfunction]
#[pyo3(signature = (**kwargs))]
pub(super) fn plan_sql_tests_natively<'py>(
    py: Python<'py>,
    kwargs: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyTuple>> {
    let args = KeywordArguments::read(
        py,
        "plan_sql_tests_natively",
        kwargs,
        (
            &[
                "project",
                "tests",
                "adapter",
                "sql_analysis_enabled",
                "render_sql",
            ],
            &["include_plan"],
        ),
    )?;
    planned(
        (&args.value("project")?, &args.value("adapter")?),
        &args.value("tests")?,
        &SqlTestPlanOptions {
            sql_analysis_enabled: args.value("sql_analysis_enabled")?,
            render_sql: args.flag("render_sql")?,
            include_plan: !args.passed("include_plan")? || args.flag("include_plan")?,
            artifact: false,
        },
    )
}

#[pyfunction]
#[pyo3(signature = (**kwargs))]
pub(crate) fn plan_compiled_sql_test_artifacts<'py>(
    py: Python<'py>,
    kwargs: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyTuple>> {
    let args = KeywordArguments::read(
        py,
        "plan_compiled_sql_test_artifacts",
        kwargs,
        (
            &["project", "tests", "adapter", "sql_analysis_enabled"],
            &[],
        ),
    )?;
    planned(
        (&args.value("project")?, &args.value("adapter")?),
        &args.value("tests")?,
        &SqlTestPlanOptions {
            sql_analysis_enabled: args.value("sql_analysis_enabled")?,
            render_sql: true,
            include_plan: false,
            artifact: true,
        },
    )
}

#[pyfunction]
#[pyo3(signature = (*, warnings))]
pub(crate) fn sql_test_plan_error_messages<'py>(
    warnings: Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyTuple>> {
    let mut messages: Vec<String> = Vec::new();
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    for warning in warnings.try_iter()? {
        let warning = warning?;
        let message: String = native_field(&warning, "message")?.extract()?;
        if native_field(&native_field(&warning, "severity")?, "value")?.extract::<String>()?
            == super::constants::ERROR_SEVERITY
            && seen.insert(message.clone())
        {
            messages.push(message);
        }
    }
    PyTuple::new(warnings.py(), messages)
}

/// Return model closures resolved natively in one project batch, keyed by SQL test key.
#[pyfunction]
#[pyo3(signature = (*, project, tests))]
pub(crate) fn sql_test_model_chain_names_by_key<'py>(
    project: &Bound<'py, PyAny>,
    tests: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyDict>> {
    let py = project.py();
    let chains = resolve_sql_test_model_chains(project, tests)?;
    let by_key = PyDict::new(py);
    for (test, chain) in tests.try_iter()?.zip(chains.try_iter()?) {
        by_key.set_item(native_field(&test?, "key")?, chain?)?;
    }
    Ok(by_key)
}
