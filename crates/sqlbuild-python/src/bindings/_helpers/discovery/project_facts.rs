//! The facts cross-file validation reads, taken from Python's `DiscoveredProjectInputs`.

use crate::bindings::_helpers::compile_context::delta::injection;
use crate::bindings::_helpers::discovery::model_file_record::{ModelFileTexts, NativeModelFile};
use crate::bindings::_helpers::records::fields::{field as native_field, path_stem};
use pyo3::FromPyObject;
use pyo3::exceptions::{PyTypeError, PyValueError};
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods, PyTuple, PyTypeMethods};
use sqlbuild_discovery::project_validation::errors::CsvDialectError;
use sqlbuild_discovery::project_validation::models::{
    AuditFileFact, CsvSettings, DependencyFact, IntegrationLoaderFact, ModelFact, NamedPath,
    NodeFact, NodeKind, ProjectFacts, SchemaFileFact, ScopedDeclarationsFact, SeedEntryFact,
    SeedFileFact, SourceFact, SourceFileFact, TestBlockFact, TestFileFact,
};

const CSV_MODULE: &str = "csv";
/// `DeclarationKind.AUDIT`: a generic audit file.
const GENERIC_AUDIT_KIND: &str = "audit";
/// Each Python node kind and the discovered inputs field that lists it, in discovery order.
const PYTHON_NODE_FIELDS: [(&str, &str); 4] = [
    ("loader", "loader_functions"),
    ("task", "task_functions"),
    ("asset", "asset_functions"),
    ("check", "check_functions"),
];
/// `ScopeKind.PRIVATE`.
const PRIVATE_SCOPE: &str = "private";

/// `(SQL resource name, owning node index, decorated name)` of one dependency.
type DependencyRow = (Option<String>, Option<usize>, Option<String>);

/// A decorated Python node as the facade describes it.
#[derive(FromPyObject)]
#[pyo3(from_item_all)]
pub(crate) struct NodeRow {
    kind: String,
    name: String,
    path: String,
    decorator_config: bool,
    depends_on: Vec<DependencyRow>,
}

/// The facts of `inputs`, with the Python nodes the facade has described.
pub(crate) fn project_facts(
    py: Python<'_>,
    inputs: &Bound<'_, PyAny>,
    nodes: Vec<NodeRow>,
) -> PyResult<ProjectFacts> {
    let mut declaration_files: Vec<ScopedDeclarationsFact> =
        each(inputs, "enum_files", declarations_fact)?;
    declaration_files.extend(each(inputs, "constant_files", declarations_fact)?);
    let mut model_schemas: Vec<NamedPath> = Vec::new();
    for schema in native_field(inputs, "model_schema_files")?.try_iter()? {
        model_schemas.extend(each(&schema?, "declarations", named)?);
    }
    Ok(ProjectFacts {
        models: each(inputs, "model_files", model_fact)?,
        sql_functions: each(inputs, "sql_function_files", stem_named)?,
        python_functions: each(inputs, "python_function_files", stem_named)?,
        sql_hooks: each(inputs, "sql_hook_files", named)?,
        seeds: each(inputs, "seed_files", seed_file_fact)?,
        schemas: each(inputs, "schema_files", |schema| schema_fact(py, schema))?,
        sources: each(inputs, "source_files", source_file_fact)?,
        tests: each(inputs, "test_files", test_file_fact)?,
        scenarios: each(inputs, "scenario_files", named)?,
        audits: each(inputs, "audit_files", audit_fact)?,
        declaration_files,
        model_schemas,
        materializations: each(inputs, "materialization_files", named)?,
        nodes: nodes.into_iter().map(node_fact).collect::<PyResult<_>>()?,
        hook_functions: each(inputs, "hook_functions", named)?,
        event_exporters: each(inputs, "event_exporters", named)?,
        providers: each(inputs, "providers", named)?,
        path_default_keys: native_field(inputs, "project_config")?
            .getattr("path_defaults")?
            .try_iter()?
            .map(|key| key?.extract())
            .collect::<PyResult<_>>()?,
        project_model_paths: injection().map(|injection| injection.project_model_paths.clone()),
        csv_field_limit: Some(csv_field_limit(py)?),
        orchestration: Vec::new(),
    })
}

/// The discovered Python nodes by kind, name and path only, as the resource-name check reads them.
pub(crate) fn named_node_rows(inputs: &Bound<'_, PyAny>) -> PyResult<Vec<NodeRow>> {
    let mut rows: Vec<NodeRow> = Vec::new();
    for (kind, name) in PYTHON_NODE_FIELDS {
        rows.extend(each(inputs, name, |node| {
            Ok(NodeRow {
                kind: kind.to_owned(),
                name: native_field(node, "name")?.extract()?,
                path: text(node, "relative_path")?,
                decorator_config: false,
                depends_on: Vec::new(),
            })
        })?);
    }
    Ok(rows)
}

/// `read` applied to each item of the iterable attribute `name` of `owner`.
pub(crate) fn each<T>(
    owner: &Bound<'_, PyAny>,
    name: &str,
    read: impl Fn(&Bound<'_, PyAny>) -> PyResult<T>,
) -> PyResult<Vec<T>> {
    let mut items: Vec<T> = Vec::new();
    for item in native_field(owner, name)?.try_iter()? {
        items.push(read(&item?)?);
    }
    Ok(items)
}

fn text(owner: &Bound<'_, PyAny>, name: &str) -> PyResult<String> {
    Ok(native_field(owner, name)?.str()?.to_string())
}

fn optional_text(owner: &Bound<'_, PyAny>, name: &str) -> PyResult<Option<String>> {
    native_field(owner, name)?.extract()
}

/// `str(Path(...).stem)` of the path attribute `name`.
pub(crate) fn stem(owner: &Bound<'_, PyAny>, name: &str) -> PyResult<String> {
    path_stem(&native_field(owner, name)?)
}

/// `(resource.name, str(resource.relative_path))`.
fn named(resource: &Bound<'_, PyAny>) -> PyResult<NamedPath> {
    Ok(NamedPath {
        name: native_field(resource, "name")?.extract()?,
        path: text(resource, "relative_path")?,
    })
}

/// `(file.file_path.stem, str(file.relative_path))`; a native model file answers from its texts.
fn stem_named(file: &Bound<'_, PyAny>) -> PyResult<NamedPath> {
    if let Ok(model_file) = file.cast::<NativeModelFile>() {
        let texts: ModelFileTexts = model_file.get().path_texts(file.py())?;
        return Ok(NamedPath {
            name: texts.name,
            path: texts.relative_path,
        });
    }
    Ok(NamedPath {
        name: stem(file, "file_path")?,
        path: text(file, "relative_path")?,
    })
}

/// `(file.relative_path.stem, str(file.relative_path))`.
fn relative_stem_named(file: &Bound<'_, PyAny>) -> PyResult<NamedPath> {
    Ok(NamedPath {
        name: stem(file, "relative_path")?,
        path: text(file, "relative_path")?,
    })
}

fn enum_value(owner: &Bound<'_, PyAny>, name: &str) -> PyResult<String> {
    native_field(&native_field(owner, name)?, "value")?.extract()
}

fn model_fact(model: &Bound<'_, PyAny>) -> PyResult<ModelFact> {
    let mut local_declarations: Vec<NamedPath> = each(model, "enum_declarations", named)?;
    local_declarations.extend(each(model, "constant_declarations", named)?);
    Ok(ModelFact {
        file: stem_named(model)?,
        local_declarations,
    })
}

fn seed_file_fact(seed: &Bound<'_, PyAny>) -> PyResult<SeedFileFact> {
    Ok(SeedFileFact {
        file: stem_named(seed)?,
        absolute_path: text(seed, "file_path")?,
    })
}

pub(crate) fn column_name(column: &Bound<'_, PyAny>) -> PyResult<String> {
    native_field(column, "name")?.extract()
}

/// The CSV reader settings a seed declaration authors, without the dialect check.
pub(crate) fn seed_csv_settings(entry: &Bound<'_, PyAny>) -> PyResult<CsvSettings> {
    let settings: Bound<'_, PyAny> = native_field(entry, "csv_settings")?;
    Ok(CsvSettings {
        delimiter: optional_text(&settings, "delimiter")?,
        quotechar: optional_text(&settings, "quotechar")?,
        escapechar: optional_text(&settings, "escapechar")?,
        doublequote: native_field(&settings, "doublequote")?.extract()?,
        skipinitialspace: native_field(&settings, "skipinitialspace")?.extract()?,
        dialect_error: None,
    })
}

fn seed_entry_fact(py: Python<'_>, entry: &Bound<'_, PyAny>) -> PyResult<SeedEntryFact> {
    let mut csv: CsvSettings = seed_csv_settings(entry)?;
    csv.dialect_error = csv_dialect_error(py, &csv)?;
    Ok(SeedEntryFact {
        name: column_name(entry)?,
        columns: each(entry, "columns", column_name)?,
        csv,
    })
}

fn schema_fact(py: Python<'_>, schema: &Bound<'_, PyAny>) -> PyResult<SchemaFileFact> {
    Ok(SchemaFileFact {
        path: text(schema, "relative_path")?,
        model_names: each(schema, "model_entries", column_name)?,
        seeds: each(schema, "seed_entries", |entry| seed_entry_fact(py, entry))?,
    })
}

fn source_fact(entry: &Bound<'_, PyAny>) -> PyResult<SourceFact> {
    let integration: Bound<'_, PyAny> = native_field(entry, "integration_loader")?;
    let integration_loader: Option<IntegrationLoaderFact> = if integration.is_none() {
        None
    } else {
        Some(IntegrationLoaderFact {
            kind: native_field(&integration, "kind")?.extract()?,
        })
    };
    Ok(SourceFact {
        name: column_name(entry)?,
        managed: native_field(entry, "managed")?.extract()?,
        loader: optional_text(entry, "loader")?,
        integration_loader,
    })
}

fn source_file_fact(file: &Bound<'_, PyAny>) -> PyResult<SourceFileFact> {
    Ok(SourceFileFact {
        path: text(file, "relative_path")?,
        entries: each(file, "source_entries", source_fact)?,
    })
}

fn test_block_fact(block: &Bound<'_, PyAny>) -> PyResult<TestBlockFact> {
    Ok(TestBlockFact {
        name: optional_text(block, "name")?,
        cases: each(block, "cases", column_name)?,
    })
}

fn test_file_fact(file: &Bound<'_, PyAny>) -> PyResult<TestFileFact> {
    Ok(TestFileFact {
        file: relative_stem_named(file)?,
        blocks: each(file, "blocks", test_block_fact)?,
    })
}

fn block_name(block: &Bound<'_, PyAny>) -> PyResult<Option<String>> {
    optional_text(block, "name")
}

fn audit_fact(file: &Bound<'_, PyAny>) -> PyResult<AuditFileFact> {
    Ok(AuditFileFact {
        file: relative_stem_named(file)?,
        generic: enum_value(file, "declaration_kind")? == GENERIC_AUDIT_KIND,
        block_names: each(file, "blocks", block_name)?,
    })
}

fn declarations_fact(file: &Bound<'_, PyAny>) -> PyResult<ScopedDeclarationsFact> {
    Ok(ScopedDeclarationsFact {
        private: enum_value(file, "scope_kind")? == PRIVATE_SCOPE,
        declarations: each(file, "declarations", named)?,
    })
}

fn node_kind(kind: &str) -> PyResult<NodeKind> {
    match kind {
        "loader" => Ok(NodeKind::Loader),
        "task" => Ok(NodeKind::Task),
        "asset" => Ok(NodeKind::Asset),
        "check" => Ok(NodeKind::Check),
        other => Err(PyValueError::new_err(format!(
            "unknown Python node kind {other:?}"
        ))),
    }
}

fn dependency_fact((sql_name, owner, decorated_name): DependencyRow) -> DependencyFact {
    match sql_name {
        Some(name) => DependencyFact::SqlResource { name },
        None => DependencyFact::Object {
            owner,
            decorated_name,
        },
    }
}

fn node_fact(node: NodeRow) -> PyResult<NodeFact> {
    Ok(NodeFact {
        kind: node_kind(&node.kind)?,
        name: node.name,
        path: node.path,
        decorator_config: node.decorator_config,
        depends_on: node.depends_on.into_iter().map(dependency_fact).collect(),
    })
}

/// The error the running Python's `csv.reader` raises for a seed's settings, which vary by version.
fn csv_dialect_error(py: Python<'_>, csv: &CsvSettings) -> PyResult<Option<CsvDialectError>> {
    let keywords: Bound<'_, PyDict> = PyDict::new(py);
    let delimiter: &str = csv
        .delimiter
        .as_deref()
        .filter(|text| !text.is_empty())
        .unwrap_or(",");
    keywords.set_item("delimiter", delimiter)?;
    keywords.set_item("quotechar", csv.quotechar.as_deref())?;
    keywords.set_item("escapechar", csv.escapechar.as_deref())?;
    keywords.set_item("doublequote", csv.doublequote.unwrap_or(true))?;
    keywords.set_item("skipinitialspace", csv.skipinitialspace.unwrap_or(false))?;
    let reader: Bound<'_, PyAny> = py.import(CSV_MODULE)?.getattr("reader")?;
    match reader.call((PyTuple::empty(py),), Some(&keywords)) {
        Ok(_) => Ok(None),
        Err(error)
            if error.is_instance_of::<PyTypeError>(py)
                || error.is_instance_of::<PyValueError>(py) =>
        {
            Ok(Some(CsvDialectError {
                class: error.get_type(py).name()?.to_string(),
                message: error.value(py).str()?.to_string(),
            }))
        }
        Err(error) => Err(error),
    }
}

fn csv_field_limit(py: Python<'_>) -> PyResult<usize> {
    py.import(CSV_MODULE)?
        .call_method0("field_size_limit")?
        .extract()
}
