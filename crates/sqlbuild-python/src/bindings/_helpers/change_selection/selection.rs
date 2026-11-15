//! Which tests and scenarios a `--since` run selects, and why, from what each really executes.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::Path;

use pyo3::exceptions::PyValueError;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use pyo3::types::{PyDict, PyDictMethods};
use serde_json::Value;
use sqlbuild_analysis::definition_changes::constants::{
    MACRO_KIND, MODEL_KIND, SCENARIO_KIND, SEED_KIND, SOURCE_KIND, TABLE_FUNCTION_KIND,
    TEST_FILE_KIND,
};
use sqlbuild_analysis::definition_changes::main::changes_summary::changes_summary;
use sqlbuild_analysis::definition_changes::main::compare_definitions::compare_definitions;
use sqlbuild_analysis::definition_changes::main::node_key::node_key;
use sqlbuild_analysis::definition_changes::main::selection_json::selection_json;
use sqlbuild_analysis::definition_changes::main::selection_reasons::selection_reasons;
use sqlbuild_analysis::definition_changes::main::selection_report::selection_report;
use sqlbuild_analysis::definition_changes::models::{
    DefinitionChanges, DefinitionSnapshot, ExecutedItem, SelectedItem,
};

use crate::bindings::_helpers::change_selection::base_snapshot::BaseComparison;
use crate::bindings::_helpers::change_selection::definition_snapshot::{
    declaration_uses, definition_snapshot, project_wide_keys,
};
use crate::bindings::_helpers::change_selection::environment_nodes::with_current_dependencies;
use crate::bindings::_helpers::compiled_views::model_reads;
use crate::bindings::_helpers::compiled_views::project_reads;
use crate::bindings::_helpers::graph::keys::native_key;
use crate::bindings::_helpers::planner_scenario::graph::plan_scenario_graph;
use crate::bindings::_helpers::scenario_command::setup::{flush, write};
use sqlbuild_python_base::records::main::fields::field::field as native_field;

const MACRO_TEST_MODE: &str = "macro";
/// The JSON document key of the changes and selection of a `--since` run.
const SINCE_SECTION: &str = "since";
/// The relation call a test or scenario file reads a source with.
const SOURCE_CALL: &str = "__source(";

/// Compiled tests or models by name.
pub(crate) type ByName<'py> = HashMap<String, Bound<'py, PyAny>>;
const TEST_CONSUMER_KIND: &str = "test";

/// The changes since the base revision and what the current revision uses.
pub(crate) struct ChangedProject {
    pub(crate) reference: String,
    merge_base: String,
    pub(crate) current: DefinitionSnapshot,
    pub(crate) changes: DefinitionChanges,
    uses: BTreeMap<String, BTreeSet<String>>,
}

/// Compare the compiled working tree `project`, discovered as `discovered`, with the base snapshot.
pub(crate) fn changed_project(
    (project, discovered): (&Bound<'_, PyAny>, &Bound<'_, PyAny>),
    project_dir: &Path,
    base: BaseComparison,
) -> PyResult<ChangedProject> {
    let current: DefinitionSnapshot = with_current_dependencies(
        project.py(),
        &base.base,
        &definition_snapshot(project, discovered, project_dir)?,
    )?;
    let changes: DefinitionChanges = compare_definitions(&base.snapshot, &current);
    Ok(ChangedProject {
        reference: base.base.reference,
        merge_base: base.base.merge_base,
        current,
        changes,
        uses: declaration_uses(project)?,
    })
}

fn strings(value: &Bound<'_, PyAny>) -> PyResult<Vec<String>> {
    value.try_iter()?.map(|item| item?.extract()).collect()
}

/// Names of the sources `sql` reads with `__source("name")`.
fn source_calls(sql: &str) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    for (index, _) in sql.match_indices(SOURCE_CALL) {
        let rest: &str = sql[index + SOURCE_CALL.len()..].trim_start();
        let Some(quote) = rest
            .chars()
            .next()
            .filter(|quote| *quote == '"' || *quote == '\'')
        else {
            continue;
        };
        if let Some(end) = rest[1..].find(quote) {
            names.push(rest[1..=end].to_owned());
        }
    }
    names
}

/// Node keys of mocked models, sources and seeds.
fn relation_keys((refs, sources, seeds): (&[String], &[String], &[String])) -> Vec<String> {
    let models = refs.iter().map(|name| node_key(MODEL_KIND, name));
    let sources = sources.iter().map(|name| node_key(SOURCE_KIND, name));
    let seeds = seeds.iter().map(|name| node_key(SEED_KIND, name));
    models.chain(sources).chain(seeds).collect()
}

/// The sources `model` reads, other than `mocked` ones, as node keys.
fn read_sources(model: &Bound<'_, PyAny>, mocked: &[String]) -> PyResult<Vec<String>> {
    let mut keys: Vec<String> = Vec::new();
    for dependency in model_reads::deps(model)?.try_iter()? {
        let (kind, name) = native_key(&dependency?)?;
        if kind == SOURCE_KIND && !mocked.contains(&name) {
            keys.push(node_key(SOURCE_KIND, &name));
        }
    }
    Ok(keys)
}

impl ChangedProject {
    /// Keys of current nodes of `kind` named in `names`.
    fn keys_named(&self, kind: &str, names: &[String]) -> BTreeSet<String> {
        self.current
            .nodes
            .iter()
            .filter(|(_, node)| node.kind == kind && names.contains(&node.name))
            .map(|(key, _)| key.clone())
            .collect()
    }

    fn declarations_of(&self, consumer: &str) -> Vec<String> {
        self.uses
            .get(consumer)
            .into_iter()
            .flatten()
            .cloned()
            .collect()
    }

    /// Why `item` is affected; empty when nothing it runs changed.
    pub(crate) fn reasons(&self, item: &ExecutedItem) -> Vec<String> {
        selection_reasons(item, &self.current, &self.changes)
    }

    /// The `since` section of the run's JSON document.
    pub(crate) fn json(&self, (total, selected): (usize, &[SelectedItem])) -> String {
        selection_json(
            (&self.reference, &self.merge_base),
            &self.changes,
            (total, selected),
        )
        .to_string()
    }

    /// Write the change summary and the selection to `stream`.
    pub(crate) fn report(
        &self,
        stream: &Bound<'_, PyAny>,
        nouns: (&str, &str),
        (total, selected, explain): (usize, &[SelectedItem], bool),
    ) -> PyResult<()> {
        write(
            stream,
            &selection_report(
                (&self.reference, &changes_summary(&self.changes)),
                nouns,
                (total, selected),
                explain,
            ),
        )?;
        flush(stream)
    }

    /// What one planned unit test executes, honouring its mocks.
    pub(crate) fn test_item(
        &self,
        entry: &Bound<'_, PyAny>,
        (tests, models): (&ByName<'_>, &ByName<'_>),
    ) -> PyResult<ExecutedItem> {
        let name: String = native_field(entry, "name")?.extract()?;
        let parent: Option<String> = native_field(entry, "parent_name")?.extract()?;
        let mut item: ExecutedItem = ExecutedItem {
            declarations: project_wide_keys(),
            ..ExecutedItem::default()
        };
        let mocked_seeds: Vec<String> = strings(&native_field(entry, "mock_seed_names")?)?;
        let mocked_sources: Vec<String> = strings(&native_field(entry, "mock_source_names")?)?;
        let mocked_refs: Vec<String> = strings(&native_field(entry, "mock_ref_names")?)?;
        let mocked_functions: Vec<String> =
            strings(&native_field(entry, "mock_table_function_names")?)?;
        item.mocked
            .extend(self.keys_named(TABLE_FUNCTION_KIND, &mocked_functions));
        item.mocked_relations = relation_keys((&mocked_refs, &mocked_sources, &mocked_seeds));
        let mut consumers: Vec<String> = vec![name.clone()];
        consumers.extend(parent.clone());
        let compiled: Option<&Bound<'_, PyAny>> = tests
            .get(&name)
            .or_else(|| parent.as_ref().and_then(|parent| tests.get(parent)));
        if let Some(test) = compiled {
            let file: Bound<'_, PyAny> = native_field(test, "test_file")?;
            let path: Bound<'_, PyAny> = native_field(&file, "relative_path")?;
            item.own_keys.push(node_key(
                TEST_FILE_KIND,
                &path.str()?.to_string().replace('\\', "/"),
            ));
            consumers.push(native_field(&path, "stem")?.extract()?);
            for block in native_field(&file, "blocks")?.try_iter()? {
                let block_name: Option<String> = native_field(&block?, "name")?.extract()?;
                consumers.extend(block_name);
            }
            let contents: String = native_field(&file, "contents")?.extract()?;
            for name in source_calls(&contents) {
                if !mocked_sources.contains(&name) {
                    item.executed.push(node_key(SOURCE_KIND, &name));
                }
            }
            let payload: Bound<'_, PyAny> = native_field(test, "payload")?;
            if let Ok(mocks) = native_field(&payload, "macro_mocks")
                && let Ok(mocks) = mocks.cast::<PyDict>()
            {
                let names: Vec<String> = mocks
                    .iter()
                    .map(|(key, _)| key.extract())
                    .collect::<PyResult<_>>()?;
                item.mocked.extend(self.keys_named(MACRO_KIND, &names));
            }
            if let Ok(mode) = native_field(&payload, "mode")
                && native_field(&mode, "value")?.extract::<String>()? == MACRO_TEST_MODE
            {
                let tested: Vec<String> =
                    strings(&native_field(&payload, "tested_resource_names")?)?;
                item.declarations
                    .extend(self.keys_named(MACRO_KIND, &tested));
            }
        }
        for step in native_field(entry, "chain")?.try_iter()? {
            let model_name: String = native_field(&step?, "model_name")?.extract()?;
            item.executed.push(node_key(MODEL_KIND, &model_name));
            let Some(model) = models.get(&model_name) else {
                continue;
            };
            item.executed.extend(read_sources(model, &mocked_sources)?);
            for dependency in model_reads::deps(model)?.try_iter()? {
                let (kind, dependency_name) = native_key(&dependency?)?;
                if kind == SEED_KIND && !mocked_seeds.contains(&dependency_name) {
                    item.executed.push(node_key(SEED_KIND, &dependency_name));
                }
            }
        }
        for key in native_field(entry, "function_deps")?.try_iter()? {
            let (kind, function_name) = native_key(&key?)?;
            item.executed.push(node_key(&kind, &function_name));
        }
        for consumer in consumers {
            item.declarations
                .extend(self.declarations_of(&node_key(TEST_CONSUMER_KIND, &consumer)));
        }
        Ok(item)
    }

    /// What one compiled scenario builds and loads, honouring its fixtures.
    pub(crate) fn scenario_item(
        &self,
        scenario: &Bound<'_, PyAny>,
        (project, models): (&Bound<'_, PyAny>, &ByName<'_>),
    ) -> PyResult<ExecutedItem> {
        let name: String = native_field(scenario, "name")?.extract()?;
        let key: String = node_key(SCENARIO_KIND, &name);
        let (graph, _) = plan_scenario_graph(scenario, project, true, None)?;
        let mut declarations: Vec<String> = project_wide_keys();
        declarations.extend(self.declarations_of(&key));
        let mut item: ExecutedItem = ExecutedItem {
            own_keys: vec![key.clone()],
            declarations,
            ..ExecutedItem::default()
        };
        let source_fixtures: Vec<String> = strings(&native_field(&graph, "source_fixture_names")?)?;
        item.mocked_relations = relation_keys((
            &strings(&native_field(&graph, "ref_fixture_names")?)?,
            &source_fixtures,
            &strings(&native_field(&graph, "seed_fixture_names")?)?,
        ));
        for name in source_calls(&native_field(scenario, "sql_body")?.extract::<String>()?) {
            if !source_fixtures.contains(&name) {
                item.executed.push(node_key(SOURCE_KIND, &name));
            }
        }
        for model_name in strings(&native_field(&graph, "model_names")?)? {
            item.executed.push(node_key(MODEL_KIND, &model_name));
            if let Some(model) = models.get(&model_name) {
                item.executed.extend(read_sources(model, &source_fixtures)?);
            }
        }
        let fixtures: Vec<String> = strings(&native_field(&graph, "seed_fixture_names")?)?;
        for seed_name in strings(&native_field(&graph, "seed_names")?)? {
            if !fixtures.contains(&seed_name) {
                item.executed.push(node_key(SEED_KIND, &seed_name));
            }
        }
        for function in native_field(&graph, "function_deps")?.try_iter()? {
            let (kind, function_name) = native_key(&function?)?;
            item.executed.push(node_key(&kind, &function_name));
        }
        Ok(item)
    }
}

/// `payload`, a JSON document, with the `since` section added when the run used `--since`.
pub(crate) fn with_since_section(payload: String, since: Option<&str>) -> PyResult<String> {
    let Some(since) = since else {
        return Ok(payload);
    };
    let invalid = |error: serde_json::Error| PyValueError::new_err(error.to_string());
    let mut document: Value = serde_json::from_str(&payload).map_err(invalid)?;
    if let Value::Object(fields) = &mut document {
        fields.insert(
            SINCE_SECTION.to_owned(),
            serde_json::from_str(since).map_err(invalid)?,
        );
    }
    Ok(format!(
        "{}\n",
        serde_json::to_string_pretty(&document).map_err(invalid)?
    ))
}

/// The project's compiled tests and models by name.
pub(crate) fn tests_and_models<'py>(
    project: &Bound<'py, PyAny>,
) -> PyResult<(ByName<'py>, ByName<'py>)> {
    let mut tests: ByName<'py> = HashMap::new();
    for test in project_reads::sql_tests(project)?.try_iter()? {
        let test: Bound<'py, PyAny> = test?;
        tests.insert(native_field(&test, "name")?.extract()?, test);
    }
    let mut models: ByName<'py> = HashMap::new();
    for model in project_reads::models(project)?.try_iter()? {
        let model: Bound<'py, PyAny> = model?;
        models.insert(model_reads::model_name(&model)?, model);
    }
    Ok((tests, models))
}
