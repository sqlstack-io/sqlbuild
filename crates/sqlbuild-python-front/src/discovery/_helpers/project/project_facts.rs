//! The facts cross-file validation reads, taken from Python's `DiscoveredProjectInputs`.

use crate::discovery::main::project::column_name::column_name;
use crate::discovery::main::project::seed_csv_settings::seed_csv_settings;
use crate::discovery::main::project::stem::stem;
use crate::discovery::models::NodeRow;
use crate::discovery::models::{ModelFileTexts, NativeModelFile};
use pyo3::exceptions::{PyTypeError, PyValueError};
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods, PyTuple, PyTypeMethods};
use sqlbuild_discovery::project_validation::errors::CsvDialectError;
use sqlbuild_discovery::project_validation::models::{
    AuditFileFact, CsvSettings, DependencyFact, ModelFact, NamedPath, NodeFact, NodeKind,
    SchemaFileFact, ScopedDeclarationsFact, SeedEntryFact, SeedFileFact, SourceFact,
    SourceFileFact, TestBlockFact, TestFileFact,
};
use sqlbuild_python_base::records::main::fields::field::field as native_field;

const CSV_MODULE: &str = "csv";
/// `DeclarationKind.AUDIT`: a generic audit file.
const GENERIC_AUDIT_KIND: &str = "audit";
/// Each Python node kind and the discovered inputs field that lists it, in discovery order.
pub(crate) const PYTHON_NODE_FIELDS: [(&str, &str); 4] = [
    ("loader", "loader_functions"),
    ("task", "task_functions"),
    ("asset", "asset_functions"),
    ("check", "check_functions"),
];
/// `ScopeKind.PRIVATE`.
const PRIVATE_SCOPE: &str = "private";

/// `(SQL resource name, owning node index, decorated name)` of one dependency.
pub(crate) type DependencyRow = (Option<String>, Option<usize>, Option<String>);

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

pub(crate) fn text(owner: &Bound<'_, PyAny>, name: &str) -> PyResult<String> {
    Ok(native_field(owner, name)?.str()?.to_string())
}

pub(crate) fn optional_text(owner: &Bound<'_, PyAny>, name: &str) -> PyResult<Option<String>> {
    native_field(owner, name)?.extract()
}

/// `(resource.name, str(resource.relative_path))`.
pub(crate) fn named(resource: &Bound<'_, PyAny>) -> PyResult<NamedPath> {
    Ok(NamedPath {
        name: native_field(resource, "name")?.extract()?,
        path: text(resource, "relative_path")?,
    })
}

/// `(file.file_path.stem, str(file.relative_path))`; a native model file answers from its texts.
pub(crate) fn stem_named(file: &Bound<'_, PyAny>) -> PyResult<NamedPath> {
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

pub(crate) fn model_fact(model: &Bound<'_, PyAny>) -> PyResult<ModelFact> {
    let mut local_declarations: Vec<NamedPath> = each(model, "enum_declarations", named)?;
    local_declarations.extend(each(model, "constant_declarations", named)?);
    Ok(ModelFact {
        file: stem_named(model)?,
        local_declarations,
    })
}

pub(crate) fn seed_file_fact(seed: &Bound<'_, PyAny>) -> PyResult<SeedFileFact> {
    Ok(SeedFileFact {
        file: stem_named(seed)?,
        absolute_path: text(seed, "file_path")?,
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

pub(crate) fn schema_fact(py: Python<'_>, schema: &Bound<'_, PyAny>) -> PyResult<SchemaFileFact> {
    Ok(SchemaFileFact {
        path: text(schema, "relative_path")?,
        model_names: each(schema, "model_entries", column_name)?,
        seeds: each(schema, "seed_entries", |entry| seed_entry_fact(py, entry))?,
    })
}

fn source_fact(entry: &Bound<'_, PyAny>) -> PyResult<SourceFact> {
    Ok(SourceFact {
        name: column_name(entry)?,
        managed: native_field(entry, "managed")?.extract()?,
        loader: optional_text(entry, "loader")?,
    })
}

pub(crate) fn source_file_fact(file: &Bound<'_, PyAny>) -> PyResult<SourceFileFact> {
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

pub(crate) fn test_file_fact(file: &Bound<'_, PyAny>) -> PyResult<TestFileFact> {
    Ok(TestFileFact {
        file: relative_stem_named(file)?,
        blocks: each(file, "blocks", test_block_fact)?,
    })
}

fn block_name(block: &Bound<'_, PyAny>) -> PyResult<Option<String>> {
    optional_text(block, "name")
}

pub(crate) fn audit_fact(file: &Bound<'_, PyAny>) -> PyResult<AuditFileFact> {
    Ok(AuditFileFact {
        file: relative_stem_named(file)?,
        generic: enum_value(file, "declaration_kind")? == GENERIC_AUDIT_KIND,
        block_names: each(file, "blocks", block_name)?,
    })
}

pub(crate) fn declarations_fact(file: &Bound<'_, PyAny>) -> PyResult<ScopedDeclarationsFact> {
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

pub(crate) fn node_fact(node: NodeRow) -> PyResult<NodeFact> {
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

pub(crate) fn csv_field_limit(py: Python<'_>) -> PyResult<usize> {
    py.import(CSV_MODULE)?
        .call_method0("field_size_limit")?
        .extract()
}
