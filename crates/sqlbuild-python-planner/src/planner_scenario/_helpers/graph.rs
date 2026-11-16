//! Convert project views into pure scenario dependency facts.

use crate::planner_scenario::_helpers::values::string;
use crate::planner_scenario::constants::SCENARIO_PLAN_GRAPH_VALIDATION;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, pyfunction};
use pyo3::types::PyTuple;
use sqlbuild_analysis::planner_scenario::main::graph::graph;
use sqlbuild_analysis::planner_scenario::models::ScenarioGraphInput;
use sqlbuild_python_base::records::main::construct::enum_member::enum_member;
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::RecordId;
use sqlbuild_python_project::compiled_views::main::compiled_models::name_object::name_object;
use sqlbuild_python_project::compiled_views::main::project::compiled_models::compiled_models;
use sqlbuild_python_project::compiled_views::main::project::seeds::seeds;
use sqlbuild_python_project::compiled_views::main::project::sources::sources;
use sqlbuild_python_project::graph::main::nodes::native_key::native_key;
use sqlbuild_python_project::graph::models::KeyCache;

#[pyfunction]
#[pyo3(signature = (*, scenario, project, sql_analysis_enabled=true, sql_analysis_dialect=None))]
pub(crate) fn plan_scenario_graph<'py>(
    scenario: &Bound<'py, PyAny>,
    project: &Bound<'py, PyAny>,
    sql_analysis_enabled: bool,
    sql_analysis_dialect: Option<&str>,
) -> PyResult<(Bound<'py, PyAny>, Bound<'py, PyTuple>)> {
    let _ = (sql_analysis_enabled, sql_analysis_dialect);
    let py = scenario.py();
    let mut input = ScenarioGraphInput {
        name: native_field(scenario, "name")?.extract()?,
        expected: native_field(scenario, "expected_model_names")?.extract()?,
        assertions: native_field(scenario, "assertion_target_model_names")?.extract()?,
        targets: native_field(scenario, "target_model_names")?.extract()?,
        refs: native_field(scenario, "ref_fixture_names")?.extract()?,
        sources: native_field(scenario, "source_fixture_names")?.extract()?,
        seeds: native_field(scenario, "seed_fixture_names")?.extract()?,
        dbt_refs: native_field(scenario, "dbt_ref_fixture_names")?.extract()?,
        ..ScenarioGraphInput::default()
    };
    for model in compiled_models(project)?.try_iter()? {
        let model = model?;
        let deps = native_field(&model, "deps")?
            .try_iter()?
            .map(|dep| native_key(&dep?))
            .collect::<PyResult<_>>()?;
        input.models.insert(name_object(&model)?.extract()?, deps);
    }
    for source in sources(project)?.try_iter()? {
        input.project_sources.insert(
            sqlbuild_python_project::resource_views::main::source_reads::name::name(&source?)?
                .extract()?,
        );
    }
    for seed in seeds(project)?.try_iter()? {
        input.project_seeds.insert(
            sqlbuild_python_project::resource_views::main::seed_reads::name::name(&seed?)?
                .extract()?,
        );
    }
    let output = graph(&input);
    input.targets.sort();
    input.assertions.sort();
    let plan = new_record_with(
        py,
        RecordId::ScenarioGraphPlan,
        &[
            ("key", native_field(scenario, "key")?),
            ("name", native_field(scenario, "name")?),
            (
                "target_model_names",
                PyTuple::new(py, input.targets)?.into_any(),
            ),
            (
                "assertion_target_model_names",
                PyTuple::new(py, input.assertions)?.into_any(),
            ),
            ("model_names", PyTuple::new(py, output.models)?.into_any()),
            (
                "source_fixture_names",
                PyTuple::new(py, output.sources)?.into_any(),
            ),
            (
                "ref_fixture_names",
                PyTuple::new(py, output.refs)?.into_any(),
            ),
            ("seed_names", PyTuple::new(py, output.seeds)?.into_any()),
            (
                "seed_fixture_names",
                PyTuple::new(py, output.seed_fixtures)?.into_any(),
            ),
            (
                "dbt_ref_fixture_names",
                PyTuple::new(py, output.dbt_refs)?.into_any(),
            ),
            (
                "function_deps",
                PyTuple::new(py, KeyCache::new(py)?.keys(&output.functions)?)?.into_any(),
            ),
        ],
    )?;
    let mut warnings: Vec<Bound<'py, PyAny>> = Vec::new();
    for (message, coded) in output.warnings {
        let mut values = vec![
            ("model_name", py.None().into_bound(py)),
            (
                "severity",
                enum_member(py, RecordId::WarningSeverity, "ERROR")?,
            ),
            ("message", string(py, &message)),
        ];
        if coded {
            values.push(("code", string(py, SCENARIO_PLAN_GRAPH_VALIDATION)));
        }
        warnings.push(new_record_with(py, RecordId::PlanWarning, &values)?);
    }
    Ok((plan, PyTuple::new(py, warnings)?))
}
