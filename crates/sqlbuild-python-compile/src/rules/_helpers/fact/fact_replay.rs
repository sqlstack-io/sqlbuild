//! Native replay of the compiler facts custom-rule evaluations recorded, for their digests.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyErr, PyResult, PyTypeMethods, Python};
use pyo3::types::{PyDict, PyDictMethods, PyString, PyStringMethods, PyType};
use sqlbuild_core::json::models::JsonValue;
use sqlbuild_rules::custom_host_pool::types::FactKey;
use sqlbuild_rules::project_tree::main::pure_path_posix::pure_path_posix;
use sqlbuild_rules::project_tree::main::pure_path_text::pure_path_text;
use sqlbuild_rules::project_tree::main::resolve_project_path::resolve_project_path;
use sqlbuild_rules::project_tree::main::tracked_text_path::tracked_text_path;
use sqlbuild_rules::project_tree::models::ProjectTreeEntry;

use crate::rules::_helpers::fact::fact_encoder::Encoded;
use crate::rules::_helpers::fact::fact_encoder::FactEncoder;
use crate::rules::_helpers::fact::fact_encoder::FactFailure;
use crate::rules::_helpers::fact::fact_encoder::error_digest;
use crate::rules::_helpers::fact::fact_encoder::value_digest;
use crate::rules::_helpers::fact::fact_tree::Outcome;
use crate::rules::_helpers::fact::fact_tree::Raised;
use crate::rules::_helpers::fact::fact_tree::TreeQuery;
use crate::rules::_helpers::fact::fact_tree::nodes_value;
use crate::rules::_helpers::fact::fact_tree::path_string;
use crate::rules::_helpers::fact::fact_tree::read_text;
use crate::rules::_helpers::fact::fact_tree::relative_parts;
use crate::rules::_helpers::fact::fact_tree::selected_nodes;
use crate::rules::_helpers::fact::fact_tree::tree_nodes;
use sqlbuild_project::types::InferredRow;
use sqlbuild_python_base::records::main::construct::record_class::record_class;
use sqlbuild_python_base::records::main::fields::field::field as record_field_or_attribute;
use sqlbuild_python_base::records::main::fields::record_spec::record_spec;
use sqlbuild_python_base::records::types::RecordId;
use sqlbuild_python_project::compiled_views::models::NativeCompiledModel;
use sqlbuild_python_project::compiled_views::models::NativeProjectView;
use sqlbuild_python_project::resource_views::models::NativeCompiledAudit;
use sqlbuild_python_project::resource_views::models::NativeCompiledFunction;
use sqlbuild_python_project::resource_views::models::NativeCompiledSeed;
use sqlbuild_python_project::resource_views::models::NativeCompiledSource;

const MODEL_RECORD: &str = "sqlbuild._native.Model";
const MATERIALIZED: &str = "materialized";
const CONTRACT: &str = "contract";
const GRAIN: &str = "grain";
const MISSING_MODEL: &str = "MissingModel";
const POSIX_SEPARATOR: char = '/';

/// One fact a custom rule can read through its context.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Fact {
    Models,
    Sources,
    Seeds,
    Functions,
    Tests,
    Audits,
    TreePaths,
    TestsAll,
    AuditsAll,
    PublicEnums,
    PublicConstants,
    Enums,
    Constants,
    SqlForModel,
    Dependencies,
    Dependents,
    Declared,
    Inferred,
    Names,
    EnumTypes,
    Enforced,
    Grain,
    TestsForModel,
    AuditsForModel,
    Children,
    Descendants,
    Glob,
    RelativeParts,
    ResourcesUnder,
    ReadText,
}

/// What a fact key carries after the fact's name.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Arguments {
    None,
    ModelPath,
    ModelName,
    Text(usize),
}

/// Every fact by key name, globals first, in the order full read sets list them.
const FACTS: [(&str, Fact, Arguments); 30] = [
    ("project.models", Fact::Models, Arguments::None),
    ("project.sources", Fact::Sources, Arguments::None),
    ("project.seeds", Fact::Seeds, Arguments::None),
    ("project.functions", Fact::Functions, Arguments::None),
    ("project.tests", Fact::Tests, Arguments::None),
    ("project.audits", Fact::Audits, Arguments::None),
    ("tree.paths", Fact::TreePaths, Arguments::None),
    ("tests.all", Fact::TestsAll, Arguments::None),
    ("audits.all", Fact::AuditsAll, Arguments::None),
    (
        "declarations.public_enums",
        Fact::PublicEnums,
        Arguments::None,
    ),
    (
        "declarations.public_constants",
        Fact::PublicConstants,
        Arguments::None,
    ),
    ("declarations.enums", Fact::Enums, Arguments::None),
    ("declarations.constants", Fact::Constants, Arguments::None),
    ("sql.for_model", Fact::SqlForModel, Arguments::ModelPath),
    (
        "graph.dependencies",
        Fact::Dependencies,
        Arguments::ModelPath,
    ),
    ("graph.dependents", Fact::Dependents, Arguments::ModelPath),
    ("columns.declared", Fact::Declared, Arguments::ModelPath),
    ("columns.inferred", Fact::Inferred, Arguments::ModelPath),
    ("columns.names", Fact::Names, Arguments::ModelPath),
    ("columns.enum_types", Fact::EnumTypes, Arguments::ModelPath),
    ("contracts.enforced", Fact::Enforced, Arguments::ModelPath),
    ("contracts.grain", Fact::Grain, Arguments::ModelPath),
    ("tests.for_model", Fact::TestsForModel, Arguments::ModelName),
    (
        "audits.for_model",
        Fact::AuditsForModel,
        Arguments::ModelName,
    ),
    ("tree.children", Fact::Children, Arguments::Text(1)),
    ("tree.descendants", Fact::Descendants, Arguments::Text(1)),
    ("tree.glob", Fact::Glob, Arguments::Text(1)),
    (
        "tree.relative_parts",
        Fact::RelativeParts,
        Arguments::Text(2),
    ),
    (
        "tree.resources_under",
        Fact::ResourcesUnder,
        Arguments::Text(1),
    ),
    ("tree.read_text", Fact::ReadText, Arguments::Text(1)),
];
const READ_TEXT_FACT: &str = "tree.read_text";

/// Whether the fact is read from the live project filesystem.
pub(crate) fn is_tree_fact(key: &FactKey) -> bool {
    key.first().is_some_and(|name| {
        FACTS.iter().any(|(fact_name, fact, arguments)| {
            fact_name == name
                && (*fact == Fact::TreePaths || matches!(arguments, Arguments::Text(_)))
        })
    })
}

/// The fact a key name reads, if it names one.
fn fact_of(name: &str) -> Option<Fact> {
    FACTS
        .iter()
        .find(|(fact_name, _, _)| *fact_name == name)
        .map(|(_, fact, _)| *fact)
}

/// Whether a delta compiling `model_paths` reads `key` exactly as a full compile would.
pub(crate) fn delta_safe_fact(
    key: &FactKey,
    model_paths: &BTreeSet<String>,
    all_sources: bool,
) -> bool {
    if is_tree_fact(key) {
        return !key
            .first()
            .is_some_and(|name| fact_of(name) == Some(Fact::ResourcesUnder));
    }
    if let [name] = key.as_slice() {
        return all_sources
            && FACTS
                .iter()
                .any(|(fact_name, fact, _)| fact_name == name && *fact == Fact::Sources);
    }
    let [name, path] = key.as_slice() else {
        return false;
    };
    FACTS.iter().any(|(fact_name, fact, arguments)| {
        fact_name == name
            && *arguments == Arguments::ModelPath
            && matches!(
                fact,
                Fact::SqlForModel
                    | Fact::Declared
                    | Fact::Inferred
                    | Fact::Names
                    | Fact::EnumTypes
                    | Fact::Enforced
                    | Fact::Grain
            )
            && model_paths.contains(path)
    })
}

/// The project file whose text `key` reads: a file's text or a model's SQL.
pub(crate) fn read_text_path(key: &FactKey) -> Option<&str> {
    let [name, path] = key.as_slice() else {
        return None;
    };
    FACTS
        .iter()
        .any(|(fact_name, fact, _)| {
            fact_name == name && matches!(fact, Fact::ReadText | Fact::SqlForModel)
        })
        .then_some(path.as_str())
}

/// What the facts read of one compiled model.
pub(crate) struct ModelEntry<'py> {
    pub(crate) model: Bound<'py, PyAny>,
    pub(crate) name: String,
    pub(crate) path: Bound<'py, PyAny>,
    path_text: String,
    pub(crate) posix: String,
    pub(crate) config_values: Option<Bound<'py, PyDict>>,
    pub(crate) key: (String, String),
    pub(crate) deps: Vec<(String, String)>,
}

/// The models in project order with the indexes the model facts look them up by.
struct Models<'py> {
    entries: Vec<ModelEntry<'py>>,
    sorted: Vec<usize>,
    by_path: HashMap<String, usize>,
    by_key: HashMap<(String, String), usize>,
    dependents: HashMap<(String, String), Vec<usize>>,
}

/// Recompute recorded facts from one compiled project snapshot and digest them.
pub(crate) struct FactReplay<'py> {
    project: Bound<'py, PyAny>,
    root: PathBuf,
    encoder: FactEncoder,
    models: Option<Models<'py>>,
    nodes: Option<Vec<ProjectTreeEntry>>,
}

impl<'py> FactReplay<'py> {
    pub(crate) fn new(project: &Bound<'py, PyAny>, project_dir: &Path) -> PyResult<Self> {
        Ok(Self {
            project: project.clone(),
            root: resolve_project_path(project_dir)?,
            encoder: FactEncoder::new(),
            models: None,
            nodes: None,
        })
    }

    /// One fact's digest, or `Unreproducible` when it cannot be recomputed.
    pub(crate) fn digest(&mut self, key: &FactKey) -> Encoded<String> {
        let py: Python<'py> = self.project.py();
        match self.outcome(key) {
            Ok(Ok(value)) => value_digest(value),
            Ok(Err(raised)) => error_digest(&raised.kind, &raised.message),
            Err(FactFailure::Python(error)) => {
                let (kind, message) = python_error(py, &error)?;
                error_digest(&kind, &message)
            }
            Err(failure) => Err(failure),
        }
    }

    /// Keys covering every fact a custom rule can observe through its context.
    pub(crate) fn full_fact_keys(&mut self) -> PyResult<Vec<FactKey>> {
        let mut keys: Vec<FactKey> = FACTS
            .iter()
            .filter(|(_, _, arguments)| *arguments == Arguments::None)
            .map(|(name, _, _)| vec![(*name).to_owned()])
            .collect();
        let models: &Models<'py> = self.models()?;
        for index in &models.sorted {
            let model: &ModelEntry<'py> = &models.entries[*index];
            for (name, _, arguments) in &FACTS {
                match arguments {
                    Arguments::ModelPath => {
                        keys.push(vec![(*name).to_owned(), model.posix.clone()])
                    }
                    Arguments::ModelName => keys.push(vec![(*name).to_owned(), model.name.clone()]),
                    Arguments::None | Arguments::Text(_) => {}
                }
            }
        }
        let root: PathBuf = self.root.clone();
        let nodes: &Vec<ProjectTreeEntry> = self.nodes.get_or_insert_with(|| tree_nodes(&root));
        for node in nodes {
            if node.is_file && tracked_text_path(&node.relative) {
                keys.push(vec![
                    READ_TEXT_FACT.to_owned(),
                    node.relative.to_string_lossy().into_owned(),
                ]);
            }
        }
        Ok(keys)
    }

    fn outcome(&mut self, key: &FactKey) -> Encoded<Outcome> {
        let Some((name, arguments)) = key.split_first() else {
            return Err(FactFailure::Unreproducible);
        };
        let Some((_, fact, expected)) = FACTS.iter().find(|(fact_name, _, expected)| {
            fact_name == name
                && match expected {
                    Arguments::None => arguments.is_empty(),
                    Arguments::ModelPath | Arguments::ModelName => arguments.len() == 1,
                    Arguments::Text(count) => arguments.len() == *count,
                }
        }) else {
            return Err(FactFailure::Unreproducible);
        };
        match expected {
            Arguments::None => self.global(*fact).map(Ok),
            Arguments::ModelPath => self.model_fact(*fact, &arguments[0]),
            Arguments::ModelName => self.named(*fact, &arguments[0]).map(Ok),
            Arguments::Text(_) => self.tree_fact(*fact, arguments),
        }
    }

    fn global(&mut self, fact: Fact) -> Encoded<JsonValue> {
        let py: Python<'py> = self.project.py();
        match fact {
            Fact::Models => {
                let sorted: Vec<usize> = self.models()?.sorted.clone();
                self.model_values(&sorted)
            }
            Fact::Sources | Fact::Seeds | Fact::Functions | Fact::Audits | Fact::AuditsAll => {
                let field: &str = match fact {
                    Fact::Sources => "sources",
                    Fact::Seeds => "seeds",
                    Fact::Functions => "functions",
                    _ => "audits",
                };
                let mut values: Vec<JsonValue> = Vec::new();
                for item in self.project_field(field)?.try_iter()? {
                    let item: Bound<'py, PyAny> = item?;
                    let (name, _, fields) = item_fields(&self.encoder, &item)?;
                    values.push(self.encoder.record(&item, &name, &fields)?);
                }
                Ok(JsonValue::Array(values))
            }
            Fact::Tests | Fact::TestsAll => {
                let tests: Bound<'py, PyAny> = self.project_field("sql_tests")?;
                self.encoder.encode(&tests)
            }
            Fact::PublicEnums | Fact::PublicConstants => {
                let field: &str = if fact == Fact::PublicEnums {
                    "public_enums"
                } else {
                    "public_constants"
                };
                let declarations: Bound<'py, PyAny> = self.project_field(field)?;
                self.encoder.encode(&declarations)
            }
            Fact::Enums | Fact::Constants => self.declarations(py, fact == Fact::Enums),
            Fact::TreePaths => {
                let root: PathBuf = self.root.clone();
                let nodes: &Vec<ProjectTreeEntry> =
                    self.nodes.get_or_insert_with(|| tree_nodes(&root));
                let nodes: Vec<&ProjectTreeEntry> = nodes.iter().collect();
                nodes_value(&self.encoder, &nodes)
            }
            _ => Err(FactFailure::Unreproducible),
        }
    }

    /// Public declarations, then each model's own, in project order.
    fn declarations(&mut self, py: Python<'py>, enums: bool) -> Encoded<JsonValue> {
        let public: Bound<'py, PyAny> = self.project_field(if enums {
            "public_enums"
        } else {
            "public_constants"
        })?;
        let mut values: Vec<JsonValue> = vec![self.encoder.encode(&public)?];
        let models: Vec<Bound<'py, PyAny>> = self
            .models()?
            .entries
            .iter()
            .map(|entry| entry.model.clone())
            .collect();
        for model in models {
            let declarations: Bound<'py, PyAny> = match model.cast::<NativeCompiledModel>() {
                Ok(view) if enums => view.get().enum_declarations(py)?.into_bound(py).into_any(),
                Ok(view) => view
                    .get()
                    .constant_declarations(py)?
                    .into_bound(py)
                    .into_any(),
                Err(_) => record_field(
                    &self.encoder,
                    &model,
                    if enums {
                        "enum_declarations"
                    } else {
                        "constant_declarations"
                    },
                )?,
            };
            values.push(self.encoder.encode(&declarations)?);
        }
        Ok(JsonValue::Array(values))
    }

    fn model_fact(&mut self, fact: Fact, path: &str) -> Encoded<Outcome> {
        let py: Python<'py> = self.project.py();
        let text: String = pure_path_text(Path::new(path))
            .to_string_lossy()
            .into_owned();
        let Some(index) = self.models()?.by_path.get(&path_key(&text)).copied() else {
            return Ok(Err(Raised::new(MISSING_MODEL, text)));
        };
        let model: Bound<'py, PyAny> = self.models()?.entries[index].model.clone();
        let view: Option<&NativeCompiledModel> = match model.cast::<NativeCompiledModel>() {
            Ok(view) => Some(view.get()),
            Err(_) => None,
        };
        let value: JsonValue = match fact {
            Fact::SqlForModel => match &view {
                Some(view) => {
                    let (authored, expanded) = view.sql_texts(py)?;
                    JsonValue::Array(vec![
                        JsonValue::String(authored),
                        JsonValue::String(expanded),
                        JsonValue::String(view.query_place(py)?),
                    ])
                }
                None => {
                    let authored: Bound<'py, PyAny> =
                        record_field(&self.encoder, &model, "authored_sql")?;
                    let expanded: Bound<'py, PyAny> =
                        record_field(&self.encoder, &model, "query_sql")?;
                    JsonValue::Array(vec![
                        self.encoder.encode(&authored)?,
                        self.encoder.encode(&expanded)?,
                    ])
                }
            },
            Fact::Dependencies => {
                let models: &Models<'py> = self.models()?;
                let indexes: Vec<usize> = models.entries[index]
                    .deps
                    .iter()
                    .filter_map(|key| models.by_key.get(key).copied())
                    .collect();
                self.model_values(&indexes)?
            }
            Fact::Dependents => {
                let models: &Models<'py> = self.models()?;
                let mut indexes: Vec<usize> = models
                    .dependents
                    .get(&models.entries[index].key)
                    .cloned()
                    .unwrap_or_default();
                indexes.sort_by(|left, right| {
                    models.entries[*left].name.cmp(&models.entries[*right].name)
                });
                self.model_values(&indexes)?
            }
            Fact::Declared => match &view {
                Some(view) => match view.declared_columns(py)? {
                    Some(columns) => self.encoder.encode(columns.bind(py).as_any())?,
                    None => JsonValue::Array(Vec::new()),
                },
                None => {
                    let entry: Bound<'py, PyAny> =
                        record_field(&self.encoder, &model, "schema_entry")?;
                    if entry.is_none() {
                        JsonValue::Array(Vec::new())
                    } else {
                        let columns: Bound<'py, PyAny> =
                            record_field(&self.encoder, &entry, "columns")?;
                        self.encoder.encode(&columns)?
                    }
                }
            },
            Fact::EnumTypes => {
                let columns: Bound<'py, PyAny> = match &view {
                    Some(view) => view.enum_columns(py)?.into_bound(py).into_any(),
                    None => record_field(&self.encoder, &model, "enum_columns")?,
                };
                let mut pairs: Vec<JsonValue> = Vec::new();
                for (column, declaration) in columns.cast::<PyDict>().map_err(PyErr::from)?.iter() {
                    let column: String = column.extract()?;
                    let name: String =
                        record_field(&self.encoder, &declaration, "name")?.extract()?;
                    pairs.push(JsonValue::Array(vec![
                        JsonValue::String(column),
                        JsonValue::String(name),
                    ]));
                }
                JsonValue::Array(pairs)
            }
            Fact::Inferred | Fact::Names => {
                let rows: Option<Vec<InferredRow>> = match &view {
                    Some(view) => view.inferred_rows(py)?,
                    None => inferred_record_rows(&self.encoder, &model)?,
                };
                match rows {
                    None => JsonValue::Null,
                    Some(rows) => JsonValue::Array(
                        rows.into_iter()
                            .map(|(name, column_type, nullability)| {
                                if fact == Fact::Names {
                                    return JsonValue::String(name);
                                }
                                JsonValue::Array(vec![
                                    JsonValue::String(name),
                                    column_type.map_or(JsonValue::Null, JsonValue::String),
                                    JsonValue::String(nullability),
                                ])
                            })
                            .collect(),
                    ),
                }
            }
            Fact::Enforced | Fact::Grain => {
                let name: &str = if fact == Fact::Enforced {
                    CONTRACT
                } else {
                    GRAIN
                };
                match &self.models()?.entries[index].config_values {
                    Some(values) => match values.get_item(name)? {
                        Some(value) => self.encoder.encode(&value)?,
                        None => JsonValue::Null,
                    },
                    None => JsonValue::Null,
                }
            }
            _ => return Err(FactFailure::Unreproducible),
        };
        Ok(Ok(value))
    }

    /// Tests targeting the model, or audits attached to it, by model name.
    fn named(&mut self, fact: Fact, name: &str) -> Encoded<JsonValue> {
        let (field, attribute) = if fact == Fact::TestsForModel {
            ("sql_tests", "target_model_names")
        } else {
            ("audits", "attached_target_name")
        };
        let mut values: Vec<JsonValue> = Vec::new();
        for item in self.project_field(field)?.try_iter()? {
            let item: Bound<'py, PyAny> = item?;
            let (qualified_name, names, fields) = item_fields(&self.encoder, &item)?;
            let Some(position) = names.iter().position(|field| *field == attribute) else {
                continue;
            };
            let target: &Bound<'py, PyAny> = &fields[position];
            let matched: bool = match target.cast::<PyString>() {
                Ok(text) => text.to_cow()? == name,
                Err(_) => target.contains(name).unwrap_or(false),
            };
            if matched {
                values.push(self.encoder.record(&item, &qualified_name, &fields)?);
            }
        }
        Ok(JsonValue::Array(values))
    }

    fn tree_fact(&mut self, fact: Fact, arguments: &[String]) -> Encoded<Outcome> {
        let root: PathBuf = self.root.clone();
        let query: TreeQuery<'_> = match fact {
            Fact::Children => TreeQuery::Children(&arguments[0]),
            Fact::Descendants => TreeQuery::Descendants(&arguments[0]),
            Fact::Glob => TreeQuery::Glob(&arguments[0]),
            Fact::RelativeParts => {
                return match relative_parts(&root, &arguments[0], &arguments[1]) {
                    Ok(parts) => Ok(Ok(JsonValue::Array(
                        parts
                            .iter()
                            .map(|part| path_string(part))
                            .collect::<Encoded<Vec<JsonValue>>>()?,
                    ))),
                    Err(raised) => Ok(Err(raised)),
                };
            }
            Fact::ResourcesUnder => return self.resources_under(&arguments[0]).map(Ok),
            _ => {
                return Ok(read_text(&root, &arguments[0]).map(JsonValue::String));
            }
        };
        let nodes: &Vec<ProjectTreeEntry> = self.nodes.get_or_insert_with(|| tree_nodes(&root));
        match selected_nodes(nodes, query) {
            Ok(selected) => nodes_value(&self.encoder, &selected).map(Ok),
            Err(raised) => Ok(Err(raised)),
        }
    }

    /// Models whose project path is beneath `path`, sorted by path.
    fn resources_under(&mut self, path: &str) -> Encoded<JsonValue> {
        let posix: String = String::from_utf8_lossy(&pure_path_posix(Path::new(path))).into_owned();
        let prefix: String = format!(
            "{}{POSIX_SEPARATOR}",
            posix.trim_end_matches(POSIX_SEPARATOR)
        );
        let models: &Models<'py> = self.models()?;
        let by_posix: BTreeMap<&str, usize> = models
            .sorted
            .iter()
            .map(|index| (models.entries[*index].posix.as_str(), *index))
            .collect();
        let indexes: Vec<usize> = by_posix
            .into_iter()
            .filter(|(relative, _)| relative.starts_with(&prefix))
            .map(|(_, index)| index)
            .collect();
        self.model_values(&indexes)
    }

    /// Each model as the public `Model` record the host builds for it.
    fn model_values(&mut self, indexes: &[usize]) -> Encoded<JsonValue> {
        let mut values: Vec<JsonValue> = Vec::with_capacity(indexes.len());
        for index in indexes {
            let (name, path, materialization) = {
                let entry: &ModelEntry<'py> = &self.models()?.entries[*index];
                let materialization: Option<String> = materialization(entry)?;
                (entry.name.clone(), entry.path.clone(), materialization)
            };
            let path: JsonValue = self.encoder.encode(&path)?;
            values.push(self.encoder.dataclass(
                MODEL_RECORD,
                vec![
                    JsonValue::String(name),
                    path,
                    materialization.map_or(JsonValue::Null, JsonValue::String),
                ],
            )?);
        }
        Ok(JsonValue::Array(values))
    }

    /// `(posix path, name, materialization)` of each model, in project order.
    pub(crate) fn model_rows(&mut self) -> PyResult<Vec<(String, String, Option<String>)>> {
        let models: &Models<'py> = self.models()?;
        let mut rows: Vec<(String, String, Option<String>)> =
            Vec::with_capacity(models.entries.len());
        for entry in &models.entries {
            rows.push((
                entry.posix.clone(),
                entry.name.clone(),
                materialization(entry)?,
            ));
        }
        Ok(rows)
    }

    /// One project field, from a native project view or a project record.
    fn project_field(&mut self, name: &str) -> PyResult<Bound<'py, PyAny>> {
        project_field(&self.encoder, &self.project, name)
    }

    fn models(&mut self) -> PyResult<&Models<'py>> {
        if self.models.is_none() {
            let models: Bound<'py, PyAny> = self.project_field("models")?;
            let mut entries: Vec<ModelEntry<'py>> = Vec::new();
            for model in models.try_iter()? {
                entries.push(model_entry(&self.encoder, &model?)?);
            }
            self.models = Some(indexed(entries));
        }
        self.models
            .as_ref()
            .ok_or_else(|| pyo3::exceptions::PyRuntimeError::new_err("models were not indexed"))
    }
}

/// One project attribute: a native project view's stored field, a record's field, or by `getattr`.
pub(crate) fn project_attribute<'py>(
    project: &Bound<'py, PyAny>,
    name: &str,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = project.py();
    if let Ok(view) = project.cast::<NativeProjectView>()
        && let Some(field) = view.get().fact_field(py, name)
    {
        return Ok(field.into_bound(py));
    }
    record_field_or_attribute(project, name)
}

/// One project field, from a native project view or a project record.
fn project_field<'py>(
    encoder: &FactEncoder,
    project: &Bound<'py, PyAny>,
    name: &str,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = project.py();
    if let Ok(view) = project.cast::<NativeProjectView>()
        && let Some(field) = view.get().fact_field(py, name)
    {
        return Ok(field.into_bound(py));
    }
    record_field(encoder, project, name)
}

/// Each model of a native project view or project record, read once without attribute lookups.
pub(crate) fn project_models<'py>(project: &Bound<'py, PyAny>) -> PyResult<Vec<ModelEntry<'py>>> {
    let encoder: FactEncoder = FactEncoder::new();
    let mut entries: Vec<ModelEntry<'py>> = Vec::new();
    for model in project_field(&encoder, project, "models")?.try_iter()? {
        entries.push(model_entry(&encoder, &model?)?);
    }
    Ok(entries)
}

/// The model's `materialized` config value when it is a string.
fn materialization(entry: &ModelEntry<'_>) -> PyResult<Option<String>> {
    let Some(values) = &entry.config_values else {
        return Ok(None);
    };
    Ok(match values.get_item(MATERIALIZED)? {
        Some(value) => match value.cast::<PyString>() {
            Ok(text) => Some(text.to_string_lossy().into_owned()),
            Err(_) => None,
        },
        None => None,
    })
}

/// The models with their lookup indexes, as the Python fact views keyed them.
fn indexed(entries: Vec<ModelEntry<'_>>) -> Models<'_> {
    let mut sorted: Vec<usize> = (0..entries.len()).collect();
    sorted.sort_by(|left, right| entries[*left].posix.cmp(&entries[*right].posix));
    let mut by_path: HashMap<String, usize> = HashMap::new();
    let mut by_key: HashMap<(String, String), usize> = HashMap::new();
    for (index, entry) in entries.iter().enumerate() {
        by_path.insert(path_key(&entry.path_text), index);
        by_key.insert(entry.key.clone(), index);
    }
    let mut dependents: HashMap<(String, String), Vec<usize>> = HashMap::new();
    for entry in &entries {
        let Some(subject) = by_key.get(&entry.key).copied() else {
            continue;
        };
        for dependency in &entry.deps {
            dependents
                .entry(dependency.clone())
                .or_default()
                .push(subject);
        }
    }
    Models {
        entries,
        sorted,
        by_path,
        by_key,
        dependents,
    }
}

/// What the facts read of one model view or `CompiledModel` record.
fn model_entry<'py>(encoder: &FactEncoder, model: &Bound<'py, PyAny>) -> PyResult<ModelEntry<'py>> {
    let py: Python<'py> = model.py();
    let (name, path, config, key, deps) = match model.cast::<NativeCompiledModel>() {
        Ok(view) => {
            let view: &NativeCompiledModel = view.get();
            (
                view.name(py)?,
                view.relative_path(py)?.into_bound(py),
                view.config(py)?.into_bound(py),
                view.key_row(py)?,
                view.dep_rows(py)?,
            )
        }
        Err(_) => {
            let key: Bound<'py, PyAny> = record_field(encoder, model, "key")?;
            let mut deps: Vec<(String, String)> = Vec::new();
            for dependency in record_field(encoder, model, "deps")?.try_iter()? {
                deps.push(key_row(encoder, &dependency?)?);
            }
            (
                record_field(encoder, model, "name")?.extract()?,
                record_field(encoder, model, "relative_path")?,
                record_field(encoder, model, "config")?,
                key_row(encoder, &key)?,
                deps,
            )
        }
    };
    let path_text: String = path.str()?.to_string_lossy().into_owned();
    let posix: String =
        String::from_utf8_lossy(&pure_path_posix(Path::new(&path_text))).into_owned();
    let config_values: Option<Bound<'py, PyDict>> = match encoder.field(&config, "values")? {
        Some(values) => Some(values.cast_into::<PyDict>().map_err(PyErr::from)?),
        None => None,
    };
    Ok(ModelEntry {
        model: model.clone(),
        name,
        path,
        path_text: pure_path_text(Path::new(&path_text))
            .to_string_lossy()
            .into_owned(),
        posix,
        config_values,
        key,
        deps,
    })
}

/// `(name, type, nullability)` of each `InferredColumn` of a `CompiledModel` record.
fn inferred_record_rows(
    encoder: &FactEncoder,
    model: &Bound<'_, PyAny>,
) -> PyResult<Option<Vec<InferredRow>>> {
    let columns: Bound<'_, PyAny> = record_field(encoder, model, "inferred_columns")?;
    if columns.is_none() {
        return Ok(None);
    }
    let mut rows: Vec<InferredRow> = Vec::new();
    for column in columns.try_iter()? {
        let column: Bound<'_, PyAny> = column?;
        let (nullability, _) = key_row_of(encoder, &column, ("nullability", "name"))?;
        rows.push((
            record_field(encoder, &column, "name")?.extract()?,
            record_field(encoder, &column, "type")?.extract()?,
            nullability,
        ));
    }
    Ok(Some(rows))
}

/// `(resource type, name)` of a `CompiledObjectKey` record.
fn key_row(encoder: &FactEncoder, key: &Bound<'_, PyAny>) -> PyResult<(String, String)> {
    key_row_of(encoder, key, ("resource_type", "name"))
}

/// A record's enum field by value, with its text field.
fn key_row_of(
    encoder: &FactEncoder,
    key: &Bound<'_, PyAny>,
    (kind, name): (&str, &str),
) -> PyResult<(String, String)> {
    let resource: Bound<'_, PyAny> = record_field(encoder, key, kind)?;
    let resource: String = match encoder.encode(&resource) {
        Ok(JsonValue::String(text)) => text,
        Ok(_) | Err(FactFailure::Unreproducible) => resource.str()?.extract()?,
        Err(FactFailure::Python(error)) => return Err(error),
    };
    Ok((resource, record_field(encoder, key, name)?.extract()?))
}

/// A record's field by name; a missing field is a type error.
fn record_field<'py>(
    encoder: &FactEncoder,
    value: &Bound<'py, PyAny>,
    name: &str,
) -> PyResult<Bound<'py, PyAny>> {
    encoder.field(value, name)?.ok_or_else(|| {
        pyo3::exceptions::PyTypeError::new_err(format!("fact source has no field {name}"))
    })
}

/// A record's qualified name, field names and field values.
type ItemFields<'py> = (String, Vec<&'static str>, Vec<Bound<'py, PyAny>>);

/// A project item's record name, field names and field values, without building the record.
fn item_fields<'py>(encoder: &FactEncoder, item: &Bound<'py, PyAny>) -> PyResult<ItemFields<'py>> {
    let py: Python<'py> = item.py();
    let (record, values): (RecordId, Vec<Bound<'py, PyAny>>) =
        if let Ok(view) = item.cast::<NativeCompiledSource>() {
            (RecordId::CompiledSource, view.get().record_values(py)?)
        } else if let Ok(view) = item.cast::<NativeCompiledSeed>() {
            (RecordId::CompiledSeed, view.get().record_values(py)?)
        } else if let Ok(view) = item.cast::<NativeCompiledFunction>() {
            (RecordId::CompiledFunction, view.get().record_values(py)?)
        } else if let Ok(view) = item.cast::<NativeCompiledAudit>() {
            (RecordId::CompiledAudit, view.get().record_values(py)?)
        } else {
            let class: Bound<'py, PyType> = item.get_type();
            let names: Vec<&'static str> = record_spec(&class)?
                .fields
                .iter()
                .map(|field| field.name)
                .collect();
            let values: Vec<Bound<'py, PyAny>> = names
                .iter()
                .map(|name| record_field(encoder, item, name))
                .collect::<PyResult<_>>()?;
            return Ok((encoder.qualified_name(&class)?, names, values));
        };
    let class: Bound<'py, PyAny> = record_class(py, record)?;
    let class: &Bound<'py, PyType> = class.cast::<PyType>()?;
    let names: Vec<&'static str> = record_spec(class)?
        .fields
        .iter()
        .map(|field| field.name)
        .collect();
    Ok((encoder.qualified_name(class)?, names, values))
}

/// The class name and `str()` of a Python exception a fact raised.
fn python_error(py: Python<'_>, error: &PyErr) -> PyResult<(String, String)> {
    Ok((
        error.get_type(py).name()?.to_string(),
        error.value(py).str()?.to_string_lossy().into_owned(),
    ))
}

/// A model path as the key `Path` equality compares by: case-insensitive on Windows.
#[cfg(windows)]
fn path_key(text: &str) -> String {
    text.to_lowercase()
}

/// A model path as the key `Path` equality compares by.
#[cfg(not(windows))]
fn path_key(text: &str) -> String {
    text.to_owned()
}
