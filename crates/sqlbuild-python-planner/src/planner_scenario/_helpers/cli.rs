//! Assemble CLI scenario plans while retaining adapter and executor boundaries.

use crate::planner_scenario::_helpers::artifacts::build_scenario_hash_index;
use crate::planner_scenario::_helpers::artifacts::build_scenario_relation_map;
use crate::planner_scenario::_helpers::execution::build_scenario_execution_plan;
use crate::planner_scenario::_helpers::graph::plan_scenario_graph;
use crate::planner_scenario::_helpers::relations::build_scenario_relation_plan;
use crate::planner_scenario::_helpers::values::string;
use crate::planner_scenario::constants::SCENARIO_PLAN_GRAPH_VALIDATION;
use crate::planner_scenario::models::ScenarioRelations;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyErr, PyResult, Python, pyfunction};
use pyo3::types::{PyDict, PyDictMethods, PyTuple};
use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::boundary::main::keywords::keywords;
use sqlbuild_python_base::boundary::models::KeywordArguments;
use sqlbuild_python_base::records::main::construct::enum_member::enum_member;
use sqlbuild_python_base::records::main::construct::error_class::error_class;
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::ErrorId;
use sqlbuild_python_base::records::types::RecordId;
use sqlbuild_python_project::compiled_views::main::compiled_models::name_object::name_object;
use sqlbuild_python_project::compiled_views::main::project::compiled_models::compiled_models;
use sqlbuild_python_project::planner_phases::main::object::object;

#[pyfunction]
#[pyo3(signature = (**kwargs))]
pub(crate) fn build_cli_scenario_plan<'py>(
    py: Python<'py>,
    kwargs: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    let args = KeywordArguments::read(
        py,
        "build_cli_scenario_plan",
        kwargs,
        (
            &[
                "scenario",
                "pipeline_result",
                "adapter",
                "project_name",
                "source_lexical_syntax",
            ],
            &[],
        ),
    )?;
    let scenario = args.value("scenario")?;
    let project = native_field(&args.value("pipeline_result")?, "project")?;
    let adapter = args.value("adapter")?;
    let (graph, warnings) = plan_scenario_graph(&scenario, &project, true, None)?;
    raise_for_warnings(warnings.as_any())?;
    let hashes = object(
        py,
        build_scenario_hash_index(
            py,
            Some(keywords(
                py,
                &[
                    ("project_name", args.value("project_name")?),
                    ("scenarios", native_field(&project, "sql_scenarios")?),
                    (
                        "run_namespace",
                        native_field(&native_field(&project, "scenario")?, "run_namespace")?,
                    ),
                ],
            )?),
        )?,
    )?;
    let mut identities: Vec<Bound<'py, PyAny>> = Vec::new();
    for (field, kind) in [
        ("source_fixture_names", "SOURCE"),
        ("ref_fixture_names", "REF"),
        ("dbt_ref_fixture_names", "DBT_REF"),
        ("seed_names", "SEED"),
        ("model_names", "MODEL"),
    ] {
        for name in native_field(&graph, field)?.try_iter()? {
            identities.push(new_record_with(
                py,
                RecordId::ScenarioArtifactIdentity,
                &[
                    (
                        "kind",
                        enum_member(py, RecordId::ScenarioArtifactKind, kind)?,
                    ),
                    ("logical_name", name?),
                ],
            )?);
        }
    }
    let map = build_scenario_relation_map(
        py,
        Some(keywords(
            py,
            &[
                ("scenario_name", native_field(&scenario, "name")?),
                (
                    "hash_prefix",
                    hashes.get_item(native_field(&scenario, "name")?)?,
                ),
                ("artifacts", PyTuple::new(py, identities)?.into_any()),
                (
                    "identifier_limit",
                    adapter.call_method0("maximum_identifier_length")?,
                ),
            ],
        )?),
    )?;
    let (database, schema) = namespace(&project, &graph)?;
    let relations = build_scenario_relation_plan(&ScenarioRelations {
        project: project.clone(),
        graph_plan: graph.clone(),
        relation_map: map,
        render_qualified_name: native_field(&adapter, "render_qualified_name")?,
        database,
        schema,
    })?;
    let result = build_scenario_execution_plan(
        py,
        Some(keywords(
            py,
            &[
                ("scenario", scenario),
                ("project", project),
                ("adapter", adapter),
                ("graph_plan", graph),
                ("relation_plan", relations),
                (
                    "source_lexical_syntax",
                    args.value("source_lexical_syntax")?,
                ),
            ],
        )?),
    )?;
    raise_for_warnings(&result.get_item(1)?)?;
    result.get_item(0)
}

fn namespace<'py>(
    project: &Bound<'py, PyAny>,
    graph: &Bound<'py, PyAny>,
) -> PyResult<(Bound<'py, PyAny>, Bound<'py, PyAny>)> {
    let py = project.py();
    let models = PyDict::new(py);
    for model in compiled_models(project)?.try_iter()? {
        let model = model?;
        models.set_item(name_object(&model)?, model)?;
    }
    for field in ["target_model_names", "model_names"] {
        for name in native_field(graph, field)?.try_iter()? {
            if let Some(model) = models.get_item(name?)? {
                let destination = sqlbuild_python_project::compiled_views::main::compiled_models::destination::destination(&model)?;
                return Ok((
                    native_field(&destination, "database")?,
                    native_field(&destination, "schema")?,
                ));
            }
        }
    }
    Ok((py.None().into_bound(py), py.None().into_bound(py)))
}

fn raise_for_warnings(warnings: &Bound<'_, PyAny>) -> PyResult<()> {
    let py = warnings.py();
    let severity = enum_member(py, RecordId::WarningSeverity, "ERROR")?;
    let mut messages: Vec<String> = Vec::new();
    let mut code = None;
    for warning in warnings.try_iter()? {
        let warning = warning?;
        if native_field(&warning, "severity")?.eq(&severity)? {
            if code.is_none() {
                let value = native_field(&warning, "code")?;
                code = Some(if value.is_truthy()? {
                    value
                } else {
                    string(py, SCENARIO_PLAN_GRAPH_VALIDATION)
                });
            }
            messages.push(native_field(&warning, "message")?.extract::<String>()?);
        }
    }
    if let Some(code) = code {
        return Err(PyErr::from_value(call_with(
            &error_class(py, ErrorId::PlannerInput)?,
            &[
                ("message", string(py, &messages.join("\n"))),
                ("code", code),
            ],
        )?));
    }
    Ok(())
}
