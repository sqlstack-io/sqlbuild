//! Assemble per-test outcomes around native planning and adapter fixture validation.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python, pyfunction};
use pyo3::types::{PyDict, PyDictMethods, PyTuple};

use crate::bindings::_helpers::compiled_views::model_reads;
use crate::bindings::_helpers::planner_strategy::fixture_context::planning_context;
use crate::bindings::_helpers::planner_strategy::models::RelationFixturePlanningContext;
use crate::bindings::_helpers::sql_test_glue::models::{SqlTestAssembly, SqlTestPlanOptions};
use crate::bindings::_helpers::sql_test_glue::plan_driver::planned as planned_tests;
use sqlbuild_python_base::boundary::models::KeywordArguments;
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::RecordId;

#[pyfunction]
#[pyo3(signature = (**kwargs))]
pub(super) fn plan_sql_tests<'py>(
    py: Python<'py>,
    kwargs: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyTuple>> {
    let args = KeywordArguments::read(
        py,
        "plan_sql_tests",
        kwargs,
        (
            &["tests", "project", "adapter"],
            &[
                "sql_analysis_enabled",
                "validate_fixtures",
                "fixture_planning_context",
            ],
        ),
    )?;
    let tests: Vec<Bound<'py, PyAny>> = args.value("tests")?.extract()?;
    assemble_sql_test_plans(
        &tests,
        (&args.value("project")?, &args.value("adapter")?),
        &SqlTestAssembly {
            sql_analysis_enabled: args.value_or_false("sql_analysis_enabled")?,
            validate_fixtures: args.flag("validate_fixtures")?,
            fixture_planning_context: args.value("fixture_planning_context")?,
        },
    )
}

/// Plan results of SQL tests, with fixture and expected-output diagnostics in place of failing plans.
pub(crate) fn assemble_sql_test_plans<'py>(
    tests: &[Bound<'py, PyAny>],
    (project, adapter): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
    assembly: &SqlTestAssembly<'py>,
) -> PyResult<Bound<'py, PyTuple>> {
    let py = project.py();
    let mut context: Option<Bound<'py, RelationFixturePlanningContext>> = None;
    let (planned, diagnostics) = if assembly.sql_analysis_enabled.is_truthy()?
        && assembly.validate_fixtures
    {
        let given = assembly.fixture_planning_context.clone();
        let shared = if given.is_none() {
            Bound::new(py, planning_context(project, None)?)?
        } else {
            given.cast_into::<RelationFixturePlanningContext>()?
        };
        let validated = super::plan_fixtures::validate(tests, (project, adapter), shared.get())?;
        context = Some(shared);
        validated
    } else {
        (tests.to_vec(), Default::default())
    };
    let plannable: Vec<_> = planned
        .into_iter()
        .enumerate()
        .filter_map(|(index, test)| (!diagnostics.contains_key(&index)).then_some(test))
        .collect();
    let plans = planned_tests(
        (project, adapter),
        &PyTuple::new(py, plannable)?.into_any(),
        &SqlTestPlanOptions {
            sql_analysis_enabled: assembly.sql_analysis_enabled.clone(),
            render_sql: false,
            include_plan: true,
            artifact: false,
        },
    )?;
    let plans: Vec<Bound<'py, PyAny>> = plans.extract()?;
    if plans.len() != tests.len() - diagnostics.len() {
        return Err(pyo3::exceptions::PyValueError::new_err(
            "zip() argument lengths differ",
        ));
    }
    let mut plans = plans.into_iter();
    let models = PyDict::new(py);
    for model in native_field(project, "models")?.try_iter()? {
        let model = model?;
        models.set_item(model_reads::name_object(&model)?, model)?;
    }
    let mut results: Vec<Bound<'py, PyAny>> = Vec::new();
    for (index, test) in tests.iter().enumerate() {
        if let Some(diagnostic) = diagnostics.get(&index) {
            results.push(new_record_with(
                py,
                RecordId::SqlTestPlanResult,
                &[
                    ("entry", py.None().into_bound(py)),
                    ("fixture_diagnostics", diagnostic.clone()),
                ],
            )?);
            continue;
        }
        let Some(plan) = plans.next() else {
            return Err(pyo3::exceptions::PyValueError::new_err(
                "missing SQL-test plan",
            ));
        };
        let mut errors: Vec<String> = native_field(&plan, "error_messages")?.extract()?;
        errors.extend(super::plan_entries::expected_diagnostics(
            test,
            &plan,
            context.as_ref().map(Bound::get),
        )?);
        if !errors.is_empty() {
            results.push(new_record_with(
                py,
                RecordId::SqlTestPlanResult,
                &[
                    ("entry", py.None().into_bound(py)),
                    ("fixture_diagnostics", PyTuple::new(py, errors)?.into_any()),
                ],
            )?);
            continue;
        }
        results.push(new_record_with(
            py,
            RecordId::SqlTestPlanResult,
            &[
                (
                    "entry",
                    super::plan_entries::entry(
                        test,
                        &plan,
                        (project, &models),
                        &assembly.sql_analysis_enabled,
                    )?,
                ),
                ("warnings", native_field(&plan, "warnings")?),
            ],
        )?);
    }
    PyTuple::new(py, results)
}
