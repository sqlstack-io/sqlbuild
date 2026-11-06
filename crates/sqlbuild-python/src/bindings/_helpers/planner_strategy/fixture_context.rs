//! The project-wide fixture planning context: relation metadata and per-model facts as Rust data.

use std::collections::{BTreeSet, HashMap};
use std::sync::{Arc, Mutex};

use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyModule, PyModuleMethods, PyResult, Python};
use pyo3::sync::PyOnceLock;
use pyo3::types::{PyDict, PyDictMethods, PyFrozenSet};
use pyo3::{pyfunction, pymethods, wrap_pyfunction};
use sqlbuild_analysis::planner_fixtures::main::fixture_schema::fixture_schema;
use sqlbuild_analysis::planner_fixtures::models::FixtureColumn;

use crate::bindings::_helpers::analysis_session::fixture_inferences::FixtureColumnInferences;
use crate::bindings::_helpers::compiled_views::model_reads;
use crate::bindings::_helpers::compiled_views::project_reads;
use crate::bindings::_helpers::compiled_views::resource_reads;
use crate::bindings::_helpers::compiled_views::seed_reads;
use crate::bindings::_helpers::compiled_views::source_reads;
use crate::bindings::_helpers::native_host::model_lookup::ModelHandle;
use crate::bindings::_helpers::planner_strategy::constants::{
    MODEL_REFERENCE_KIND, SEED_REFERENCE_KIND, SOURCE_REFERENCE_KIND,
};
use crate::bindings::_helpers::planner_strategy::fixture_metadata::model_fixture_relation;
use crate::bindings::_helpers::planner_strategy::fixture_reads::lineage_reads;
use crate::bindings::_helpers::planner_strategy::fixture_types::projections;
use crate::bindings::_helpers::planner_strategy::fixture_values::{columns, folded_name};
use crate::bindings::_helpers::planner_strategy::models::{
    FixtureReadCacheHost, FixtureRelation, ModelFixtureFacts, RelationFixturePlanningContext,
};
use crate::bindings::_helpers::planner_strategy::types::{FixtureKey, FixtureRelationInput};
use crate::bindings::_helpers::records::fields::field as native_field;

static FIXTURE_READS: PyOnceLock<Py<FixtureReadCacheHost>> = PyOnceLock::new();

/// Count one batch resolution of SQL-test model chains.
pub(crate) fn count_chain_resolution(py: Python<'_>) -> PyResult<()> {
    work_counts(py)?.borrow_mut().chain_resolutions += 1;
    Ok(())
}

/// The extension-wide fixture read cache and planning work counters.
pub(crate) fn work_counts(py: Python<'_>) -> PyResult<Bound<'_, FixtureReadCacheHost>> {
    Ok(FIXTURE_READS
        .get_or_try_init(py, || Py::new(py, FixtureReadCacheHost::default()))?
        .bind(py)
        .clone())
}

/// The planning context of a project, over its models or the given name-to-model map.
pub(crate) fn planning_context(
    project: &Bound<'_, PyAny>,
    models_by_name: Option<Bound<'_, PyDict>>,
) -> PyResult<RelationFixturePlanningContext> {
    let py = project.py();
    work_counts(py)?.borrow_mut().context_builds += 1;
    let model_map: HashMap<String, ModelHandle> = match models_by_name {
        Some(models) => models
            .iter()
            .map(|(name, model)| Ok((name.extract()?, ModelHandle::read(&model))))
            .collect::<PyResult<_>>()?,
        None => {
            let mut models: HashMap<String, ModelHandle> = HashMap::new();
            for model in project_reads::models(project)?.try_iter()? {
                let model = model?;
                models.insert(
                    model_reads::name_object(&model)?.extract()?,
                    ModelHandle::read(&model),
                );
            }
            models
        }
    };
    let mut relations: HashMap<FixtureKey, FixtureRelation> = HashMap::new();
    for (name, model) in &model_map {
        relations.insert(
            (MODEL_REFERENCE_KIND.to_owned(), name.clone()),
            model_fixture_relation(&model.bind(py))?,
        );
    }
    for (kind, source) in [(SOURCE_REFERENCE_KIND, true), (SEED_REFERENCE_KIND, false)] {
        let resources: Bound<'_, PyAny> = if source {
            project_reads::sources(project)?
        } else {
            project_reads::seeds(project)?
        };
        for resource in resources.try_iter()? {
            let resource = resource?;
            let entry: Bound<'_, PyAny> = if source {
                source_reads::source_entry(&resource)?
            } else {
                seed_reads::schema_entry(&resource)?
            };
            let columns: Vec<FixtureColumn> = columns(&native_field(&entry, "columns")?, false)?;
            let authoritative = if source {
                native_field(&entry, "contract")?.eq("enforced")?
            } else {
                !columns.is_empty()
            };
            relations.insert(
                (kind.to_owned(), resource_reads::name(&resource)?.extract()?),
                FixtureRelation {
                    columns,
                    authoritative,
                },
            );
        }
    }
    Ok(context_of(
        model_map,
        relations,
        Py::new(py, FixtureColumnInferences::new())?.into_any(),
    ))
}

fn context_of(
    models_by_name: HashMap<String, ModelHandle>,
    relations: HashMap<FixtureKey, FixtureRelation>,
    fixture_inferences: Py<PyAny>,
) -> RelationFixturePlanningContext {
    let mut authoritative_columns: HashMap<FixtureKey, BTreeSet<String>> = HashMap::new();
    let mut expected_types: HashMap<FixtureKey, Vec<(String, String)>> = HashMap::new();
    for (key, relation) in &relations {
        if let Some(names) =
            fixture_schema(&relation.columns, relation.authoritative).authoritative_names
        {
            authoritative_columns.insert(key.clone(), names);
        }
        expected_types.insert(
            key.clone(),
            fixture_schema(&relation.columns, false).expected_types,
        );
    }
    RelationFixturePlanningContext {
        models_by_name,
        relations,
        authoritative_columns,
        expected_types,
        fixture_inferences,
        model_facts: Mutex::new(HashMap::new()),
    }
}

impl RelationFixturePlanningContext {
    /// One model's lineage reads and type projections, derived once per context.
    pub(crate) fn model_facts(
        &self,
        py: Python<'_>,
        name: &str,
    ) -> PyResult<Option<Arc<ModelFixtureFacts>>> {
        if let Some(found) = self.facts_cache()?.get(name) {
            return Ok(Some(Arc::clone(found)));
        }
        let Some(model) = self.models_by_name.get(name) else {
            return Ok(None);
        };
        let model = model.bind(py);
        let facts = Arc::new(ModelFixtureFacts {
            reads: lineage_reads(&model)?,
            projections: projections(&model)?,
        });
        self.facts_cache()?
            .insert(name.to_owned(), Arc::clone(&facts));
        Ok(Some(facts))
    }

    fn facts_cache(
        &self,
    ) -> PyResult<std::sync::MutexGuard<'_, HashMap<String, Arc<ModelFixtureFacts>>>> {
        self.model_facts
            .lock()
            .map_err(|_| pyo3::exceptions::PyRuntimeError::new_err("fixture model facts"))
    }
}

#[pymethods]
impl RelationFixturePlanningContext {
    /// A context over explicit relation metadata: `{key: ((name, type, nullable), ...), authoritative}`.
    #[new]
    #[pyo3(signature = (*, models_by_name, relations))]
    fn new(
        py: Python<'_>,
        models_by_name: Bound<'_, PyDict>,
        relations: HashMap<FixtureKey, FixtureRelationInput>,
    ) -> PyResult<Self> {
        let mut known: HashMap<FixtureKey, FixtureRelation> = HashMap::new();
        for (key, (columns, authoritative)) in relations {
            let mut fixture_columns: Vec<FixtureColumn> = Vec::with_capacity(columns.len());
            for (name, sql_type, nullable) in columns {
                fixture_columns.push(FixtureColumn {
                    folded_name: folded_name(&name),
                    name,
                    sql_type,
                    nullable,
                });
            }
            known.insert(
                key,
                FixtureRelation {
                    columns: fixture_columns,
                    authoritative,
                },
            );
        }
        Ok(context_of(
            models_by_name
                .iter()
                .map(|(name, model)| Ok((name.extract()?, ModelHandle::read(&model))))
                .collect::<PyResult<_>>()?,
            known,
            Py::new(py, FixtureColumnInferences::new())?.into_any(),
        ))
    }

    /// Whether the relation's columns form an authoritative shape, `None` for unknown relations.
    fn authoritative_names(&self, key: FixtureKey) -> Option<bool> {
        self.relations
            .get(&key)
            .map(|relation| relation.authoritative)
    }

    /// The relation's known column types by name, `None` for unknown relations.
    fn expected_types<'py>(
        &self,
        py: Python<'py>,
        key: FixtureKey,
    ) -> PyResult<Option<Bound<'py, PyDict>>> {
        let Some(types) = self.expected_types.get(&key) else {
            return Ok(None);
        };
        let result = PyDict::new(py);
        for (name, sql_type) in types {
            result.set_item(name, sql_type)?;
        }
        Ok(Some(result))
    }

    /// The relation's authoritative column names, `None` when the shape is not authoritative.
    fn authoritative_columns<'py>(
        &self,
        py: Python<'py>,
        key: FixtureKey,
    ) -> PyResult<Option<Bound<'py, PyFrozenSet>>> {
        self.authoritative_columns
            .get(&key)
            .map(|names| PyFrozenSet::new(py, names))
            .transpose()
    }
}

/// Build the project-wide fixture planning context once for many SQL-test plans.
#[pyfunction]
#[pyo3(signature = (*, project, models_by_name = None))]
fn build_relation_fixture_context(
    project: &Bound<'_, PyAny>,
    models_by_name: Option<Bound<'_, PyDict>>,
) -> PyResult<RelationFixturePlanningContext> {
    planning_context(project, models_by_name)
}

/// Planning contexts built and SQL-test chain batches resolved, optionally resetting both.
#[pyfunction]
#[pyo3(signature = (*, reset = false))]
fn fixture_planning_work_info(py: Python<'_>, reset: bool) -> PyResult<(u64, u64)> {
    let host = work_counts(py)?;
    let mut host = host.borrow_mut();
    let counts = (host.context_builds, host.chain_resolutions);
    if reset {
        host.context_builds = 0;
        host.chain_resolutions = 0;
    }
    Ok(counts)
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<RelationFixturePlanningContext>()?;
    module.add_function(wrap_pyfunction!(build_relation_fixture_context, module)?)?;
    module.add_function(wrap_pyfunction!(fixture_planning_work_info, module)?)
}
