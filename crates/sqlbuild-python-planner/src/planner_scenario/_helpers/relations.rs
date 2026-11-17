//! Build scenario relation locations through adapter qualification callbacks.

use crate::planner_scenario::_helpers::values::error;
use crate::planner_scenario::constants::REF;
use crate::planner_scenario::constants::SCENARIO_PLAN_INTERNAL;
use crate::planner_scenario::constants::SCENARIO_PLAN_MISSING_RELATION_TARGET;
use crate::planner_scenario::constants::SOURCE;
use crate::planner_scenario::models::ScenarioRelations;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python, pyfunction};
use pyo3::types::{PyDict, PyDictMethods};
use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::boundary::models::KeywordArguments;
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::main::fields::replaced_keywords::replaced_keywords;
use sqlbuild_python_base::records::types::RecordId;
use sqlbuild_python_project::resource_views::main::source_reads::source_entry::source_entry;

#[pyfunction(name = "build_scenario_relation_plan")]
#[pyo3(signature = (**kwargs))]
pub(crate) fn build_scenario_relation_plan_of<'py>(
    py: Python<'py>,
    kwargs: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    let args = KeywordArguments::read(
        py,
        "build_scenario_relation_plan",
        kwargs,
        (
            &[
                "project",
                "graph_plan",
                "relation_map",
                "render_qualified_name",
            ],
            &["database", "schema"],
        ),
    )?;
    build_scenario_relation_plan(&ScenarioRelations {
        project: args.value("project")?,
        graph_plan: args.value("graph_plan")?,
        relation_map: args.value("relation_map")?,
        render_qualified_name: args.value("render_qualified_name")?,
        database: args.value("database")?,
        schema: args.value("schema")?,
    })
}

/// The scenario's model, seed, source and fixture locations under its relation map.
pub(crate) fn build_scenario_relation_plan<'py>(
    args: &ScenarioRelations<'py>,
) -> PyResult<Bound<'py, PyAny>> {
    let py = args.project.py();
    let graph = &args.graph_plan;
    let artifacts = PyDict::new(py);
    for artifact in native_field(&args.relation_map, "artifacts")?.try_iter()? {
        let artifact = artifact?;
        let identity = native_field(&artifact, "identity")?;
        artifacts.set_item(
            (
                native_field(&identity, "kind")?,
                native_field(&identity, "logical_name")?,
            ),
            native_field(&artifact, "physical_name")?,
        )?;
    }
    let sources = PyDict::new(py);
    for source in
        sqlbuild_python_project::compiled_views::main::project::sources::sources(&args.project)?
            .try_iter()?
    {
        let source = source?;
        sources.set_item(
            sqlbuild_python_project::resource_views::main::source_reads::name::name(&source)?,
            source_entry(&source)?,
        )?;
    }
    let models = PyDict::new(py);
    let refs = PyDict::new(py);
    let source_fixtures = PyDict::new(py);
    let dbt_refs = PyDict::new(py);
    let seeds = PyDict::new(py);
    let seed_fixtures = PyDict::new(py);
    let source_map = PyDict::new(py);
    for (field, kind, locations) in [
        ("model_names", "model", &models),
        ("ref_fixture_names", "ref", &refs),
        ("source_fixture_names", "source", &source_fixtures),
        ("dbt_ref_fixture_names", "dbt_ref", &dbt_refs),
        ("seed_names", "seed", &seeds),
    ] {
        for name in native_field(graph, field)?.try_iter()? {
            let name: String = name?.extract()?;
            let target = target_for_artifact(args, &artifacts, kind, &name)?;
            locations.set_item(&name, &target)?;
            if kind == REF {
                models.set_item(&name, &target)?;
            } else if kind == SOURCE {
                let entry = sources
                    .get_item(&name)?
                    .ok_or_else(|| pyo3::exceptions::PyKeyError::new_err(name.clone()))?;
                let changes = PyDict::new(py);
                for field in ["database", "schema"] {
                    changes.set_item(field, native_field(&target, field)?)?;
                }
                changes.set_item("table", native_field(&target, "name")?)?;
                changes.set_item("expression", py.None())?;
                changes.set_item("type_enforcement", false)?;
                let replacement = replaced_keywords(&entry, &changes)?;
                source_map.set_item(&name, replacement)?;
            }
        }
    }
    for name in native_field(graph, "seed_fixture_names")?.try_iter()? {
        let name: String = name?.extract()?;
        let Some(target) = seeds.get_item(&name)? else {
            return Err(error(
                py,
                &format!("Scenario relation plan is missing seed target '{name}'"),
                SCENARIO_PLAN_MISSING_RELATION_TARGET,
                Some("This is likely a SQLBuild bug. Please file an issue with the scenario name."),
            )?);
        };
        seed_fixtures.set_item(name, target)?;
    }
    new_record_with(
        py,
        RecordId::ScenarioRelationPlan,
        &[
            ("scenario_name", native_field(graph, "name")?),
            ("relation_map", args.relation_map.clone()),
            ("model_locations", models.into_any()),
            ("seed_locations", seeds.into_any()),
            ("project_source_map", sources.into_any()),
            ("source_map", source_map.into_any()),
            ("source_fixture_locations", source_fixtures.into_any()),
            ("ref_fixture_locations", refs.into_any()),
            ("dbt_ref_fixture_locations", dbt_refs.into_any()),
            ("seed_fixture_locations", seed_fixtures.into_any()),
        ],
    )
}

fn target_for_artifact<'py>(
    args: &ScenarioRelations<'py>,
    artifacts: &Bound<'py, PyDict>,
    kind: &str,
    name: &str,
) -> PyResult<Bound<'py, PyAny>> {
    let py = artifacts.py();
    let Some(physical) = artifacts.get_item((kind, name))? else {
        return Err(error(
            py,
            &format!("Scenario relation map is missing {kind} artifact '{name}'"),
            SCENARIO_PLAN_INTERNAL,
            Some("This is likely a SQLBuild bug. Please file an issue with the scenario name."),
        )?);
    };
    let database = args.database.clone();
    let schema = args.schema.clone();
    let qualified = call_with(
        &args.render_qualified_name,
        &[
            ("database", database.clone()),
            ("schema", schema.clone()),
            ("name", physical.clone()),
        ],
    )?;
    new_record_with(
        py,
        RecordId::CompiledRelationLocation,
        &[
            ("database", database),
            ("schema", schema),
            ("name", physical.clone()),
            (
                "qualified_name",
                if qualified.is_none() {
                    physical
                } else {
                    qualified
                },
            ),
        ],
    )
}
