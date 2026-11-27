//! The plan facts build progress rows need: positions, name width and model row labels.

use std::collections::HashMap;

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_core::build_progress::main::progress_name_width::progress_name_width;
use sqlbuild_core::build_progress::models::{ModelRowFacts, NameWidthInputs};
use sqlbuild_core::plan_display::main::model_execution_annotation::model_execution_annotation;
use sqlbuild_core::plan_display::main::model_resource_type::model_resource_type;
use sqlbuild_core::plan_display::models::ModelLabelFacts;

use crate::build_progress::_helpers::result_text::text;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_planner::plan_output::main::optional_facts::optional_facts;

const SOURCE_TYPE: &str = "source";
const SQL_TEST_TYPE: &str = "sql_test";

/// What a build run's progress rows read from its plan.
#[derive(Debug)]
pub(crate) struct BuildPlanFacts {
    pub(crate) total: usize,
    /// Display positions of every progress row, for the progress projector.
    pub(crate) ordinals: Vec<(String, usize)>,
    pub(crate) test_names: Vec<String>,
    pub(crate) name_width: usize,
    pub(crate) models: HashMap<String, ModelRowFacts>,
    /// The labels of a result whose model is not in the plan.
    pub(crate) unplanned_model: ModelRowFacts,
}

fn row_facts(facts: Option<&ModelLabelFacts>, logical_ddl: Option<String>) -> ModelRowFacts {
    ModelRowFacts {
        display_type: model_resource_type(facts).to_owned(),
        annotation: model_execution_annotation(facts),
        logical_ddl,
    }
}

fn names(plan: &Bound<'_, PyAny>, field: &str) -> PyResult<Vec<String>> {
    native_field(plan, field)?
        .try_iter()?
        .map(|entry| text(&entry?, "name"))
        .collect()
}

/// Whether the execution-order key is a source with a loader.
fn loaded_source(plan: &Bound<'_, PyAny>, name: &Bound<'_, PyAny>) -> PyResult<bool> {
    let source: Bound<'_, PyAny> =
        native_field(plan, "source_map")?.call_method1("get", (name,))?;
    Ok(!source.is_none() && !native_field(&source, "loader")?.is_none())
}

fn test_tree_children(plan: &Bound<'_, PyAny>) -> PyResult<Vec<String>> {
    let mut children: Vec<String> = Vec::new();
    for entry in native_field(plan, "test_entries")?.try_iter()? {
        let entry: Bound<'_, PyAny> = entry?;
        for step in native_field(&entry, "chain")?.try_iter()? {
            let step: Bound<'_, PyAny> = step?;
            if native_field(&step, "expected_cte_sql")?.is_truthy()? {
                children.push(text(&step, "model_name")?);
            }
        }
        for assertion in native_field(&entry, "assertions")?.try_iter()? {
            children.push(format!("assertion {}", text(&assertion?, "name")?));
        }
    }
    Ok(children)
}

pub(crate) fn build_plan_facts(plan: &Bound<'_, PyAny>) -> PyResult<BuildPlanFacts> {
    let mut models: HashMap<String, ModelRowFacts> = HashMap::new();
    let mut row_names: Vec<String> = Vec::new();
    for entry in native_field(plan, "model_entries")?.try_iter()? {
        let entry: Bound<'_, PyAny> = entry?;
        let name: String = text(&entry, "name")?;
        let facts: ModelRowFacts = row_facts(
            optional_facts(&entry)?.as_ref(),
            Some(text(&entry, "logical_ddl")?),
        );
        row_names.push(if facts.annotation.is_empty() {
            name.clone()
        } else {
            format!("{name}  ({})", facts.annotation)
        });
        models.insert(name, facts);
    }
    let model_count: usize = models.len();
    let seeds: Vec<String> = names(plan, "seed_entries")?;
    let functions: Vec<String> = names(plan, "function_entries")?;
    let test_names: Vec<String> = names(plan, "test_entries")?;
    let loads: usize = names(plan, "load_entries")?.len();
    let mut ordinals: Vec<(String, usize)> = Vec::new();
    let mut loaded_sources: usize = 0;
    for key in native_field(plan, "execution_order")?.try_iter()? {
        let key: Bound<'_, PyAny> = key?;
        let resource_type: String = text(&key, "resource_type")?;
        let name: Bound<'_, PyAny> = native_field(&key, "name")?;
        if resource_type == SQL_TEST_TYPE {
            continue;
        }
        if resource_type == SOURCE_TYPE {
            if !loaded_source(plan, &name)? {
                continue;
            }
            loaded_sources += 1;
        }
        ordinals.push((name.str()?.to_string(), ordinals.len() + 1));
    }
    row_names.extend(seeds.iter().cloned());
    row_names.extend(functions.iter().cloned());
    row_names.extend(test_names.iter().cloned());
    Ok(BuildPlanFacts {
        total: model_count + seeds.len() + functions.len() + loaded_sources + loads,
        ordinals,
        name_width: progress_name_width(&NameWidthInputs {
            names: row_names,
            tree_child_names: test_tree_children(plan)?,
        }),
        test_names,
        models,
        unplanned_model: row_facts(None, None),
    })
}
