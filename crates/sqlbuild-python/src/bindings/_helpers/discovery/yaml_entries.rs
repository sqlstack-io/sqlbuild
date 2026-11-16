//! Loaded schema and source YAML documents parsed natively into the Python contract objects.

use pyo3::prelude::{
    Bound, PyAny, PyAnyMethods, PyErr, PyModule, PyModuleMethods, PyResult, Python,
};
use pyo3::types::{PyDict, PyDictMethods, PyTuple};
use pyo3::{FromPyObject, pyfunction, wrap_pyfunction};
use sqlbuild_discovery::yaml_entries::main::parse_schema_document::parse_schema_document as parse_schema;
use sqlbuild_discovery::yaml_entries::main::parse_sources_document::parse_sources_document as parse_sources;
use sqlbuild_discovery::yaml_entries::models::{
    ParsedCsvSettings, ParsedDltSource, ParsedFreshness, ParsedIngestrConfig,
    ParsedIntegrationLoader, ParsedNaValues, ParsedSeedEntry, ParsedSourceEntry, ParsedYamlColumn,
};
use sqlbuild_model_config::errors::{ConfigError, ErrorClass};

use crate::bindings::_helpers::model_config::authored_nodes::PyNode;
use crate::bindings::_helpers::model_config::contract_classes::yaml_entry_classes;
use crate::bindings::_helpers::model_config::header_metadata::{ContractClasses, audit_tuple};
use sqlbuild_python_base::records::main::construct::error_class::error_class;
use sqlbuild_python_base::records::types::ErrorId;

/// The directory whose schema YAML files may declare seeds.
const SEEDS_DIRECTORY: &str = "seeds";
const SCHEMA_PARSE_ERROR: ErrorId = ErrorId::SchemaParse;
const SOURCE_PARSE_ERROR: ErrorId = ErrorId::SourceParse;

/// The Python classes parsed YAML entries become.
#[derive(FromPyObject)]
#[pyo3(from_item_all)]
pub(crate) struct YamlClasses<'py> {
    contracts: ContractClasses<'py>,
    schema_seed_entry: Bound<'py, PyAny>,
    seed_csv_settings: Bound<'py, PyAny>,
    source_entry: Bound<'py, PyAny>,
    source_column_entry: Bound<'py, PyAny>,
    source_freshness_config: Bound<'py, PyAny>,
    source_freshness_age_policy: Bound<'py, PyAny>,
    integration_loader_config: Bound<'py, PyAny>,
    ingestr_source_config: Bound<'py, PyAny>,
    dlt_source_config: Bound<'py, PyAny>,
    dlt_resource_config: Bound<'py, PyAny>,
    source_write_strategy: Bound<'py, PyAny>,
    source_freshness_strategy: Bound<'py, PyAny>,
    source_freshness_value_kind: Bound<'py, PyAny>,
}

/// The Python error `error` raises: a resource identity error, else the file kind's parse error.
fn parse_error(py: Python<'_>, error: ConfigError, parse_class: ErrorId) -> PyResult<PyErr> {
    let class: Bound<'_, PyAny> = match error.class {
        ErrorClass::ResourceIdentity => error_class(py, ErrorId::ResourceIdentity)?,
        _ => error_class(py, parse_class)?,
    };
    let keywords: Bound<'_, PyDict> = PyDict::new(py);
    if let Some(help) = error.help {
        keywords.set_item("help", help)?;
    }
    Ok(PyErr::from_value(
        class.call((error.message,), Some(&keywords))?,
    ))
}

fn optional<'py>(py: Python<'py>, node: Option<&PyNode<'py>>) -> Bound<'py, PyAny> {
    node.map_or_else(|| py.None().into_bound(py), |node| node.0.clone())
}

fn meta<'py>(py: Python<'py>, node: Option<&PyNode<'py>>) -> Bound<'py, PyAny> {
    node.map_or_else(|| PyDict::new(py).into_any(), |node| node.0.clone())
}

fn texts<'py>(py: Python<'py>, nodes: &[PyNode<'py>]) -> PyResult<Bound<'py, PyTuple>> {
    PyTuple::new(py, nodes.iter().map(|node| node.0.clone()))
}

pub(crate) fn yaml_columns<'py>(
    py: Python<'py>,
    (classes, column_class): (&ContractClasses<'py>, &Bound<'py, PyAny>),
    columns: &[ParsedYamlColumn<PyNode<'py>>],
) -> PyResult<Bound<'py, PyTuple>> {
    let mut built: Vec<Bound<'py, PyAny>> = Vec::with_capacity(columns.len());
    for column in columns {
        let fields: Bound<'py, PyDict> = PyDict::new(py);
        fields.set_item("name", &column.name.0)?;
        fields.set_item("type", optional(py, column.column_type.as_ref()))?;
        fields.set_item("nullable", optional(py, column.nullable.as_ref()))?;
        fields.set_item("description", optional(py, column.description.as_ref()))?;
        fields.set_item("meta", meta(py, column.meta.as_ref()))?;
        fields.set_item("audits", audit_tuple(py, classes, &column.audits)?)?;
        built.push(column_class.call((), Some(&fields))?);
    }
    PyTuple::new(py, built)
}

fn csv_settings<'py>(
    py: Python<'py>,
    class: &Bound<'py, PyAny>,
    settings: Option<&ParsedCsvSettings<PyNode<'py>>>,
) -> PyResult<Bound<'py, PyAny>> {
    let fields: Bound<'py, PyDict> = PyDict::new(py);
    if let Some(settings) = settings {
        for (key, value) in [
            ("delimiter", &settings.delimiter),
            ("quotechar", &settings.quotechar),
            ("doublequote", &settings.doublequote),
            ("escapechar", &settings.escapechar),
            ("skipinitialspace", &settings.skipinitialspace),
            ("lineterminator", &settings.lineterminator),
            ("encoding", &settings.encoding),
            ("keep_default_na", &settings.keep_default_na),
        ] {
            fields.set_item(key, optional(py, value.as_ref()))?;
        }
        let na_values: Bound<'py, PyAny> = match &settings.na_values {
            None => py.None().into_bound(py),
            Some(ParsedNaValues::List(values)) => texts(py, values)?.into_any(),
            Some(ParsedNaValues::PerColumn(columns)) => {
                let mapping: Bound<'py, PyDict> = PyDict::new(py);
                for (column, values) in columns {
                    mapping.set_item(&column.0, texts(py, values)?)?;
                }
                mapping.into_any()
            }
        };
        fields.set_item("na_values", na_values)?;
    }
    class.call((), Some(&fields))
}

fn seed_entry<'py>(
    py: Python<'py>,
    classes: &YamlClasses<'py>,
    seed: &ParsedSeedEntry<PyNode<'py>>,
) -> PyResult<Bound<'py, PyAny>> {
    let fields: Bound<'py, PyDict> = PyDict::new(py);
    fields.set_item("name", &seed.name.0)?;
    fields.set_item("description", optional(py, seed.description.as_ref()))?;
    fields.set_item("database", optional(py, seed.database.as_ref()))?;
    fields.set_item("schema", optional(py, seed.schema.as_ref()))?;
    fields.set_item("meta", meta(py, seed.meta.as_ref()))?;
    fields.set_item(
        "csv_settings",
        csv_settings(py, &classes.seed_csv_settings, seed.csv_settings.as_ref())?,
    )?;
    fields.set_item(
        "columns",
        yaml_columns(
            py,
            (&classes.contracts, &classes.contracts.schema_column),
            &seed.columns,
        )?,
    )?;
    fields.set_item("tags", texts(py, &seed.tags)?)?;
    fields.set_item("audits", audit_tuple(py, &classes.contracts, &seed.audits)?)?;
    classes.schema_seed_entry.call((), Some(&fields))
}

/// The seed entries of one loaded schema YAML document, or the parse error Python raises.
#[pyfunction]
pub(crate) fn parse_schema_document<'py>(
    py: Python<'py>,
    loaded: Bound<'py, PyAny>,
    path: &str,
) -> PyResult<Bound<'py, PyTuple>> {
    let classes: YamlClasses<'py> = yaml_entry_classes(py)?.extract()?;
    let in_seeds_directory: bool = std::path::Path::new(path)
        .components()
        .any(|component| component.as_os_str() == SEEDS_DIRECTORY);
    let seeds: Vec<ParsedSeedEntry<PyNode<'py>>> =
        parse_schema(&PyNode(loaded), path, in_seeds_directory).map_err(|error| {
            parse_error(py, error, SCHEMA_PARSE_ERROR).unwrap_or_else(|error| error)
        })?;
    let mut built: Vec<Bound<'py, PyAny>> = Vec::with_capacity(seeds.len());
    for seed in &seeds {
        built.push(seed_entry(py, &classes, seed)?);
    }
    PyTuple::new(py, built)
}

fn strings<'py>(py: Python<'py>, values: &[String]) -> PyResult<Bound<'py, PyTuple>> {
    PyTuple::new(py, values)
}

fn freshness<'py>(
    py: Python<'py>,
    classes: &YamlClasses<'py>,
    freshness: &ParsedFreshness<PyNode<'py>>,
) -> PyResult<Bound<'py, PyAny>> {
    let fields: Bound<'py, PyDict> = PyDict::new(py);
    fields.set_item(
        "strategy",
        classes
            .source_freshness_strategy
            .call1((freshness.strategy,))?,
    )?;
    let value_kind: Bound<'py, PyAny> = match freshness.value_kind {
        Some(kind) => classes.source_freshness_value_kind.call1((kind,))?,
        None => py.None().into_bound(py),
    };
    fields.set_item("value_kind", value_kind)?;
    fields.set_item("column", optional(py, freshness.column.as_ref()))?;
    fields.set_item("query", optional(py, freshness.query.as_ref()))?;
    fields.set_item("filter", optional(py, freshness.filter.as_ref()))?;
    fields.set_item(
        "lag_tolerance",
        optional(py, freshness.lag_tolerance.as_ref()),
    )?;
    let age_policy: Bound<'py, PyAny> = match &freshness.age_policy {
        Some(policy) => {
            let policy_fields: Bound<'py, PyDict> = PyDict::new(py);
            policy_fields.set_item("warn_after", optional(py, policy.warn_after.as_ref()))?;
            policy_fields.set_item("error_after", optional(py, policy.error_after.as_ref()))?;
            classes
                .source_freshness_age_policy
                .call((), Some(&policy_fields))?
        }
        None => py.None().into_bound(py),
    };
    fields.set_item("age_policy", age_policy)?;
    classes.source_freshness_config.call((), Some(&fields))
}

fn ingestr_config<'py>(
    py: Python<'py>,
    classes: &YamlClasses<'py>,
    config: &ParsedIngestrConfig<PyNode<'py>>,
) -> PyResult<Bound<'py, PyAny>> {
    let fields: Bound<'py, PyDict> = PyDict::new(py);
    fields.set_item("source_uri", &config.source_uri.0)?;
    fields.set_item("source_table", &config.source_table.0)?;
    fields.set_item("strategy", optional(py, config.strategy.as_ref()))?;
    fields.set_item(
        "incremental_key",
        optional(py, config.incremental_key.as_ref()),
    )?;
    fields.set_item("primary_key", strings(py, &config.primary_key)?)?;
    fields.set_item("columns", optional(py, config.columns.as_ref()))?;
    fields.set_item("extra_args", strings(py, &config.extra_args)?)?;
    classes.ingestr_source_config.call((), Some(&fields))
}

fn dlt_config<'py>(
    py: Python<'py>,
    classes: &YamlClasses<'py>,
    source: &ParsedDltSource<PyNode<'py>>,
) -> PyResult<Bound<'py, PyAny>> {
    let resource = &source.resource;
    let raw_config: Bound<'py, PyDict> = PyDict::new(py);
    for (key, value) in &resource.raw_config {
        raw_config.set_item(&key.0, &value.0)?;
    }
    let resource_fields: Bound<'py, PyDict> = PyDict::new(py);
    resource_fields.set_item("name", &resource.name.0)?;
    resource_fields.set_item("dlt_name", &resource.dlt_name.0)?;
    resource_fields.set_item("schema", optional(py, resource.schema.as_ref()))?;
    resource_fields.set_item("raw_config", raw_config)?;
    resource_fields.set_item(
        "write_disposition",
        optional(py, resource.write_disposition.as_ref()),
    )?;
    resource_fields.set_item("primary_key", optional(py, resource.primary_key.as_ref()))?;
    resource_fields.set_item("merge_key", optional(py, resource.merge_key.as_ref()))?;
    resource_fields.set_item("incremental", meta(py, resource.incremental.as_ref()))?;
    let fields: Bound<'py, PyDict> = PyDict::new(py);
    fields.set_item("source_type", &source.source_type.0)?;
    fields.set_item("schema", optional(py, source.schema.as_ref()))?;
    fields.set_item("config", &source.config.0)?;
    fields.set_item("destination", meta(py, source.destination.as_ref()))?;
    fields.set_item(
        "resource",
        classes
            .dlt_resource_config
            .call((), Some(&resource_fields))?,
    )?;
    fields.set_item("group_index", source.group_index)?;
    classes.dlt_source_config.call((), Some(&fields))
}

fn source_entry<'py>(
    py: Python<'py>,
    classes: &YamlClasses<'py>,
    source: &ParsedSourceEntry<PyNode<'py>>,
) -> PyResult<Bound<'py, PyAny>> {
    let integration: Bound<'py, PyAny> = match &source.integration_loader {
        None => py.None().into_bound(py),
        Some(loader) => {
            let config: Bound<'py, PyAny> = match loader {
                ParsedIntegrationLoader::Ingestr(config) => ingestr_config(py, classes, config)?,
                ParsedIntegrationLoader::Dlt(source) => dlt_config(py, classes, source)?,
            };
            classes
                .integration_loader_config
                .call1((loader.kind(), config))?
        }
    };
    let fields: Bound<'py, PyDict> = PyDict::new(py);
    fields.set_item("name", &source.name.0)?;
    fields.set_item("database", optional(py, source.database.as_ref()))?;
    fields.set_item("schema", optional(py, source.schema.as_ref()))?;
    fields.set_item("table", optional(py, source.table.as_ref()))?;
    fields.set_item("final", source.read_final)?;
    fields.set_item("loader", source.loader.as_deref())?;
    fields.set_item("managed", source.managed)?;
    fields.set_item("integration_loader", integration)?;
    let freshness_config: Bound<'py, PyAny> = match &source.freshness {
        Some(config) => freshness(py, classes, config)?,
        None => py.None().into_bound(py),
    };
    fields.set_item("freshness", freshness_config)?;
    let write_strategy: Bound<'py, PyAny> = match source.write_strategy {
        Some(strategy) => classes.source_write_strategy.call1((strategy,))?,
        None => py.None().into_bound(py),
    };
    fields.set_item("write_strategy", write_strategy)?;
    fields.set_item(
        "load_batch_size",
        optional(py, source.load_batch_size.as_ref()),
    )?;
    fields.set_item("cursor_column", optional(py, source.cursor_column.as_ref()))?;
    fields.set_item("unique_key", strings(py, &source.unique_key)?)?;
    fields.set_item("expression", optional(py, source.expression.as_ref()))?;
    fields.set_item("description", optional(py, source.description.as_ref()))?;
    fields.set_item("type_enforcement", source.type_enforcement)?;
    fields.set_item("contract", optional(py, source.contract.as_ref()))?;
    fields.set_item("meta", meta(py, source.meta.as_ref()))?;
    fields.set_item(
        "columns",
        yaml_columns(
            py,
            (&classes.contracts, &classes.source_column_entry),
            &source.columns,
        )?,
    )?;
    fields.set_item(
        "audits",
        audit_tuple(py, &classes.contracts, &source.audits)?,
    )?;
    classes.source_entry.call((), Some(&fields))
}

/// The source entries of one loaded sources YAML document, or the parse error Python raises.
#[pyfunction]
pub(crate) fn parse_sources_document<'py>(
    py: Python<'py>,
    loaded: Bound<'py, PyAny>,
    path: &str,
) -> PyResult<Bound<'py, PyTuple>> {
    let classes: YamlClasses<'py> = yaml_entry_classes(py)?.extract()?;
    let sources: Vec<ParsedSourceEntry<PyNode<'py>>> = parse_sources(&PyNode(loaded), path)
        .map_err(|error| {
            parse_error(py, error, SOURCE_PARSE_ERROR).unwrap_or_else(|error| error)
        })?;
    let referenced: Option<std::collections::BTreeSet<String>> =
        sqlbuild_python_base::compile_context::main::delta::injection::injection()
            .and_then(|injection| injection.source_names.clone());
    sqlbuild_python_base::compile_context::main::delta::record_source_reads::record_source_reads(
        loaded_sources(&sources).iter().map(String::as_str),
    );
    let mut built: Vec<Bound<'py, PyAny>> = Vec::with_capacity(sources.len());
    for source in &sources {
        if let Some(referenced) = &referenced
            && let Ok(name) = source.name.0.extract::<String>()
            && !referenced.contains(&name)
        {
            continue;
        }
        built.push(source_entry(py, &classes, source)?);
    }
    PyTuple::new(py, built)
}

/// The sources a loader fills, which own that loader wherever the project is compiled.
fn loaded_sources(sources: &[ParsedSourceEntry<PyNode<'_>>]) -> Vec<String> {
    sources
        .iter()
        .filter(|source| {
            source.managed || source.loader.is_some() || source.integration_loader.is_some()
        })
        .filter_map(|source| {
            let Ok(name) = source.name.0.extract::<String>() else {
                return None;
            };
            Some(name)
        })
        .collect()
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(parse_schema_document, module)?)?;
    module.add_function(wrap_pyfunction!(parse_sources_document, module)?)
}
