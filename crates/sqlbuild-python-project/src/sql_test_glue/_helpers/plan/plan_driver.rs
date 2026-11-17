//! Host SQL-test request preparation, native planning, and Python result conversion.

use crate::sql_test_glue::main::planned_rows::planned_rows;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyErr, PyResult, Python, pyfunction};
use pyo3::types::{PyDict, PyTuple};

use crate::planner_phases::main::object::object;
use crate::planner_strategy::main::count_chain_resolution::count_chain_resolution;
use crate::sql_test_glue::_helpers::plan::plan_values::tuple;
use crate::sql_test_glue::_helpers::plan::planning::ChainRequest;
use crate::sql_test_glue::_helpers::plan::planning::resolve_compiled_sql_test_chains;
use crate::sql_test_glue::models::PlanRow;
use crate::sql_test_glue::models::SqlTestPlanOptions;
use sqlbuild_python_base::boundary::models::KeywordArguments;
use sqlbuild_python_base::records::main::construct::error_class::error_class;
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::ErrorId;
use sqlbuild_python_base::records::types::RecordId;

impl PlanRow {
    /// Rendered SQL for an artifact, or the planning error shared by native and Python consumers.
    pub fn rendered_sql(&self, py: Python<'_>) -> PyResult<&str> {
        let Some(sql) = self.plan.sql.as_deref() else {
            return Err(PyErr::from_value(
                error_class(py, ErrorId::NativeSqlTestPlanning)?
                    .call1(("native SQL-test planning omitted rendered SQL",))?,
            ));
        };
        Ok(sql)
    }
}

pub(crate) fn chains<'py>(
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
            return Err(
                crate::sql_test_glue::_helpers::plan::plan_errors::planning_error(
                    py, error, tests,
                )?,
            );
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
        let plan = crate::sql_test_glue::_helpers::plan::plan_rows::plan(py, row)?;
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
pub(crate) fn plan_sql_tests_natively<'py>(
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
            == crate::sql_test_glue::constants::ERROR_SEVERITY
            && seen.insert(message.clone())
        {
            messages.push(message);
        }
    }
    PyTuple::new(warnings.py(), messages)
}
