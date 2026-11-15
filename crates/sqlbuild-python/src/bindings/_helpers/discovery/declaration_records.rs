//! Declaration file collections discovered natively and built as Python discovery records.

use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyModule, PyModuleMethods, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods, PyTuple};
use pyo3::{IntoPyObjectExt, pyfunction, wrap_pyfunction};
use sqlbuild_discovery::declaration_files::main::discover_declaration_collection::discover_declaration_collection;
use sqlbuild_discovery::declaration_files::main::retained_collection::retained_collection;
use sqlbuild_discovery::declaration_files::models::{
    AuditBlock, AuditFile, CollectionFailure, CollectionKind, CollectionRequest, ConstantFile,
    DeclarationCollection, DiscoverySession, EnumFile, FileScope, FunctionFile, HookFile,
    MacroFile, SchemaFile, ScopedFile,
};
use sqlbuild_discovery::declaration_files::types::Collection;
use sqlbuild_discovery::models::{FileOutcome, LineColumnSpan};
use sqlbuild_discovery::tree::models::ProjectTree;
use sqlbuild_host::entries::models::FacadeHost;
use sqlbuild_sqltext::compiler::models::AuthoredValue;
use std::sync::Arc;

use crate::bindings::_helpers::discovery::faults::{FileFaults, record_tuple};
use crate::bindings::_helpers::discovery::header_declarations::typed_constant_record;
use crate::bindings::_helpers::discovery::project_files::path_object;
use crate::bindings::_helpers::discovery::raised::{failure_error, read_error, stage_error};
use crate::bindings::_helpers::discovery::snapshot::{
    in_snapshot, project_root, project_tree, snapshot_memo,
};
use crate::bindings::_helpers::model_config::header_columns::schema_columns;
use crate::bindings::_helpers::sqltext::authored_values::map_to_python;
use crate::bindings::models::{NativeDiscoverySession, NativeProjectTree};
use sqlbuild_discovery::declaration_files::main::declaration_file_options::declaration_file_options;
use sqlbuild_python_base::boundary::main::imported::imported;
use sqlbuild_python_base::boundary::main::keywords::keywords;
use sqlbuild_python_base::native_host::main::host_entry::host_entry;
use sqlbuild_python_base::records::main::construct::enum_from_value::enum_from_value;
use sqlbuild_python_base::records::main::construct::new_record::new_record;
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use sqlbuild_python_base::records::main::fields::mapping_get::mapping_get;
use sqlbuild_python_base::records::types::RecordId;

const PATHLIB_MODULE: &str = "pathlib";
const SESSION_MEMO_KEY: &str = "native_discovery_session";
const MEASUREMENT_MODE: &str = "MEASUREMENT";

/// One declaration file being built: its paths and the scope native discovery listed it in.
struct FileContext<'a, 'py> {
    tree: &'a ProjectTree,
    relative_path: Bound<'py, PyAny>,
    file_path: Bound<'py, PyAny>,
    scope: Option<&'a FileScope>,
}

impl<'py> FileContext<'_, 'py> {
    fn py(&self) -> Python<'py> {
        self.relative_path.py()
    }

    /// The fields every declaration file record starts with.
    fn fields(&self, contents: &str) -> PyResult<Bound<'py, PyDict>> {
        keywords(
            self.py(),
            &[
                ("file_path", self.file_path.clone()),
                ("relative_path", self.relative_path.clone()),
                ("contents", contents.into_bound_py_any(self.py())?),
            ],
        )
    }

    /// Add the scope fields of a file native discovery listed in a role.
    fn add_scope(&self, fields: &Bound<'py, PyDict>) -> PyResult<()> {
        match self.scope {
            Some(scope) => add_scope_fields(self.tree, scope, fields),
            None => Ok(()),
        }
    }

    fn record(&self, class: RecordId, fields: &Bound<'py, PyDict>) -> PyResult<Bound<'py, PyAny>> {
        new_record(self.py(), class, (), Some(fields))
    }
}

/// Set `scope_kind`, `ownership_root`, `owning_path` and `declaration_root` of a listed file.
pub(crate) fn add_scope_fields(
    tree: &ProjectTree,
    scope: &FileScope,
    fields: &Bound<'_, PyDict>,
) -> PyResult<()> {
    let py: Python<'_> = fields.py();
    fields.set_item(
        "scope_kind",
        enum_from_value(py, RecordId::ScopeKind, scope.scope_kind)?,
    )?;
    for (name, path) in [
        ("ownership_root", &scope.ownership_root),
        ("owning_path", &scope.owning_path),
        ("declaration_root", &scope.declaration_root),
    ] {
        let value: Bound<'_, PyAny> = match path {
            Some(path) => path_class(py)?.call1((path_object(py, tree, path.clone())?,))?,
            None => py.None().into_bound(py),
        };
        fields.set_item(name, value)?;
    }
    Ok(())
}

pub(crate) fn path_class(py: Python<'_>) -> PyResult<Bound<'_, PyAny>> {
    imported(py, PATHLIB_MODULE, "Path")
}

fn authored<'py>(
    py: Python<'py>,
    values: &[(String, AuthoredValue)],
) -> PyResult<Bound<'py, PyDict>> {
    Ok(map_to_python(py, values)?
        .into_bound(py)
        .downcast_into::<PyDict>()?)
}

/// `{key: value}` or `{}`, read back as Python reads the projected value.
fn single_value<'py>(
    py: Python<'py>,
    key: &str,
    value: Option<&AuthoredValue>,
) -> PyResult<Option<Bound<'py, PyAny>>> {
    let values: Vec<(String, AuthoredValue)> = value
        .map(|value| vec![(key.to_owned(), value.clone())])
        .unwrap_or_default();
    authored(py, &values)?.get_item(key)
}

fn enum_file<'py>(context: &FileContext<'_, 'py>, file: &EnumFile) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = context.py();
    let mut declarations: Vec<Bound<'py, PyAny>> = Vec::with_capacity(file.declarations.len());
    for declaration in &file.declarations {
        let members: Vec<Bound<'py, PyAny>> = authored(py, &declaration.members)?
            .iter()
            .map(|(name, value)| {
                new_record_with(
                    py,
                    RecordId::EnumMember,
                    &[("name", name), ("value", value)],
                )
            })
            .collect::<PyResult<_>>()?;
        declarations.push(new_record_with(
            py,
            RecordId::EnumDeclaration,
            &[
                ("name", declaration.name.as_str().into_bound_py_any(py)?),
                ("members", PyTuple::new(py, members)?.into_any()),
                (
                    "scalar_type",
                    declaration.scalar_type.into_bound_py_any(py)?,
                ),
                ("relative_path", context.relative_path.clone()),
            ],
        )?);
    }
    let fields: Bound<'py, PyDict> = context.fields(&file.contents)?;
    fields.set_item("declarations", PyTuple::new(py, declarations)?)?;
    context.add_scope(&fields)?;
    context.record(RecordId::DiscoveredEnumFile, &fields)
}

fn constant_file<'py>(
    context: &FileContext<'_, 'py>,
    file: &ConstantFile,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = context.py();
    let mut declarations: Vec<Bound<'py, PyAny>> = Vec::with_capacity(file.declarations.len());
    for declaration in &file.declarations {
        let value: Bound<'py, PyAny> = single_value(py, "value", Some(&declaration.value))?
            .unwrap_or_else(|| py.None().into_bound(py));
        declarations.push(typed_constant_record(
            py,
            (declaration.name.clone(), value),
            (
                declaration.explicit_type.clone(),
                declaration.render_as.clone(),
            ),
            (context.file_path.clone(), context.relative_path.clone()),
        )?);
    }
    if let Some(failure) = &file.failure {
        return Err(failure_error(py, failure.clone()));
    }
    let fields: Bound<'py, PyDict> = context.fields(&file.contents)?;
    fields.set_item("declarations", PyTuple::new(py, declarations)?)?;
    context.add_scope(&fields)?;
    context.record(RecordId::DiscoveredConstantFile, &fields)
}

/// Python's `native_locations`: a repeated name keeps its first position and last value.
pub(crate) fn source_locations<'py>(
    py: Python<'py>,
    relative_path: &Bound<'py, PyAny>,
    rows: &[(String, LineColumnSpan)],
) -> PyResult<Bound<'py, PyDict>> {
    let located: Bound<'py, PyDict> = PyDict::new(py);
    for (name, span) in rows {
        located.set_item(
            name,
            new_record_with(
                py,
                RecordId::SourceLocation,
                &[
                    ("path", relative_path.clone()),
                    ("line", span.line.into_bound_py_any(py)?),
                    ("column", span.column.into_bound_py_any(py)?),
                    ("end_line", span.end_line.into_bound_py_any(py)?),
                    ("end_column", span.end_column.into_bound_py_any(py)?),
                ],
            )?,
        )?;
    }
    Ok(located)
}

fn schema_file<'py>(
    context: &FileContext<'_, 'py>,
    file: &SchemaFile,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = context.py();
    let file_path: String = context.file_path.str()?.to_string();
    let mut declarations: Vec<Bound<'py, PyAny>> = Vec::with_capacity(file.declarations.len());
    for declaration in &file.declarations {
        let raw_columns: Bound<'py, PyAny> =
            single_value(py, "columns", declaration.columns.as_ref())?
                .unwrap_or_else(|| py.None().into_bound(py));
        let label: String = format!("schema '{}'", declaration.name);
        let columns: Bound<'py, PyAny> = schema_columns(
            &raw_columns,
            (&file_path, &label),
            (true, false, true),
            &source_locations(py, &context.relative_path, &declaration.column_locations)?,
        )?
        .into_any();
        declarations.push(new_record_with(
            py,
            RecordId::ModelSchemaDeclaration,
            &[
                ("name", declaration.name.as_str().into_bound_py_any(py)?),
                (
                    "description",
                    declaration.description.as_deref().into_bound_py_any(py)?,
                ),
                (
                    "extends",
                    declaration.extends.as_deref().into_bound_py_any(py)?,
                ),
                ("columns", columns),
                ("relative_path", context.relative_path.clone()),
            ],
        )?);
    }
    if let Some(failure) = &file.failure {
        return Err(failure_error(py, failure.clone()));
    }
    let fields: Bound<'py, PyDict> = context.fields(&file.contents)?;
    fields.set_item("declarations", PyTuple::new(py, declarations)?)?;
    context.add_scope(&fields)?;
    context.record(RecordId::DiscoveredModelSchemaFile, &fields)
}

fn function_file<'py>(
    context: &FileContext<'_, 'py>,
    file: &FunctionFile,
) -> PyResult<Bound<'py, PyAny>> {
    let fields: Bound<'py, PyDict> = context.fields(&file.contents)?;
    fields.set_item(
        "header_values",
        authored(context.py(), &file.header_values)?,
    )?;
    fields.set_item("body_sql", &file.body_sql)?;
    context.record(RecordId::DiscoveredSqlFunctionFile, &fields)
}

fn hook_file<'py>(context: &FileContext<'_, 'py>, file: &HookFile) -> PyResult<Bound<'py, PyAny>> {
    let fields: Bound<'py, PyDict> = context.fields(&file.contents)?;
    fields.set_item(
        "header_values",
        authored(context.py(), &file.header_values)?,
    )?;
    fields.set_item("sql_body", &file.sql_body)?;
    fields.set_item("name", &file.name)?;
    fields.set_item("description", &file.description)?;
    context.add_scope(&fields)?;
    context.record(RecordId::DiscoveredSqlHookFile, &fields)
}

fn audit_block<'py>(py: Python<'py>, block: &AuditBlock) -> PyResult<Bound<'py, PyAny>> {
    let header_values: Bound<'py, PyDict> = authored(py, &block.header_values)?;
    let mode: Bound<'py, PyAny> =
        host_entry(py, FacadeHost::AuditEvaluationMode)?.call1((block.evaluation_mode,))?;
    let measurement: Bound<'py, PyAny> =
        if mode.is(&host_entry(py, FacadeHost::AuditEvaluationMode)?.getattr(MEASUREMENT_MODE)?) {
            new_record_with(
                py,
                RecordId::MeasurementContract,
                &[
                    ("value_column", header_values.as_any().get_item("value")?),
                    (
                        "sample_count_column",
                        mapping_get(header_values.as_any(), "sample_count")?,
                    ),
                    (
                        "sample_unit",
                        mapping_get(header_values.as_any(), "sample_unit")?,
                    ),
                ],
            )?
        } else {
            py.None().into_bound(py)
        };
    new_record_with(
        py,
        RecordId::DiscoveredAuditBlock,
        &[
            ("audit_index", block.audit_index.into_bound_py_any(py)?),
            ("header_values", header_values.into_any()),
            ("sql_body", block.sql_body.as_str().into_bound_py_any(py)?),
            ("name", block.name.as_deref().into_bound_py_any(py)?),
            ("evaluation_mode", mode),
            ("measurement_contract", measurement),
            (
                "measure_sql",
                block.measure_sql.as_deref().into_bound_py_any(py)?,
            ),
            (
                "evidence_sql",
                block.evidence_sql.as_deref().into_bound_py_any(py)?,
            ),
        ],
    )
}

fn audit_file<'py>(
    context: &FileContext<'_, 'py>,
    file: &AuditFile,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = context.py();
    let blocks: Vec<Bound<'py, PyAny>> = file
        .blocks
        .iter()
        .map(|block| audit_block(py, block))
        .collect::<PyResult<_>>()?;
    let fields: Bound<'py, PyDict> = context.fields(&file.contents)?;
    fields.set_item("blocks", PyTuple::new(py, blocks)?)?;
    let declaration_kind: &str = context.scope.map_or("", |scope| scope.declaration_kind);
    fields.set_item(
        "declaration_kind",
        enum_from_value(py, RecordId::DeclarationKind, declaration_kind)?,
    )?;
    context.add_scope(&fields)?;
    context.record(RecordId::DiscoveredAuditFile, &fields)
}

fn macro_file<'py>(
    context: &FileContext<'_, 'py>,
    file: &MacroFile,
) -> PyResult<Bound<'py, PyAny>> {
    let fields: Bound<'py, PyDict> = context.fields(&file.contents)?;
    context.add_scope(&fields)?;
    context.record(RecordId::DiscoveredMacroFile, &fields)
}

/// The session of this pass, created and memoised by the first declaration collection.
pub(crate) fn discovery_session<'py>(
    project_dir: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, NativeDiscoverySession>> {
    let py: Python<'py> = project_dir.py();
    let memo: Bound<'py, PyDict> = snapshot_memo(project_dir)?;
    if let Some(session) = memo.get_item(SESSION_MEMO_KEY)?
        && let Ok(session) = session.downcast_into::<NativeDiscoverySession>()
    {
        return Ok(session);
    }
    let created: Bound<'py, NativeDiscoverySession> = Bound::new(
        py,
        NativeDiscoverySession {
            inner: DiscoverySession::new(project_root(project_dir)?, declaration_file_options()),
        },
    )?;
    memo.set_item(SESSION_MEMO_KEY, &created)?;
    Ok(created)
}

/// The session this discovery pass read its declaration files with, if any.
#[pyfunction]
#[pyo3(signature = (*, project_dir))]
pub(crate) fn retained_discovery_session<'py>(
    project_dir: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = project_dir.py();
    Ok(snapshot_memo(project_dir)?
        .get_item(SESSION_MEMO_KEY)?
        .filter(|session| session.is_instance_of::<NativeDiscoverySession>())
        .unwrap_or_else(|| py.None().into_bound(py)))
}

/// One collection read in the pass's snapshot, with the tree it was read from.
fn collection<'py>(
    project_dir: &Bound<'py, PyAny>,
    request: CollectionRequest,
) -> PyResult<(Bound<'py, NativeProjectTree>, Arc<DeclarationCollection>)> {
    in_snapshot(project_dir, || {
        let py: Python<'py> = project_dir.py();
        let tree: Bound<'py, NativeProjectTree> = project_tree(project_dir)?;
        let session: Bound<'py, NativeDiscoverySession> = discovery_session(project_dir)?;
        let collection: Arc<DeclarationCollection> = {
            let session: &DiscoverySession = &session.get().inner;
            let inner: &ProjectTree = &tree.get().inner;
            py.detach(|| {
                retained_collection(session, request, || {
                    discover_declaration_collection(session, inner, request)
                })
            })
        };
        if let Some(failure) = collection_failure(&collection) {
            return Err(match failure {
                CollectionFailure::Stage(failure) => {
                    stage_error(&tree.get().inner, project_dir, failure.clone())
                }
                CollectionFailure::Layout(failure) => failure_error(py, failure.clone()),
            });
        }
        Ok((tree, collection))
    })
}

/// The files of a collection whose failure was already raised.
fn files_of<T>(collection: &Collection<T>) -> &[ScopedFile<T>] {
    collection.as_ref().map(Vec::as_slice).unwrap_or_default()
}

fn failure_of<T>(collection: &Collection<T>) -> Option<&CollectionFailure> {
    collection.as_ref().err()
}

fn collection_failure(collection: &DeclarationCollection) -> Option<&CollectionFailure> {
    match collection {
        DeclarationCollection::Enums(files) => failure_of(files),
        DeclarationCollection::Constants(files) => failure_of(files),
        DeclarationCollection::ModelSchemas(files) => failure_of(files),
        DeclarationCollection::SqlFunctions(files) => failure_of(files),
        DeclarationCollection::SqlHooks(files) => failure_of(files),
        DeclarationCollection::Audits(files) => failure_of(files),
        DeclarationCollection::Seeds(files) => failure_of(files),
        DeclarationCollection::Macros(files) => failure_of(files),
    }
}

/// Build each parsed file of a collection, raising or reporting each failing one.
fn records<'py, T>(
    faults: &FileFaults<'py>,
    tree: &ProjectTree,
    files: &[ScopedFile<T>],
    build: fn(&FileContext<'_, 'py>, &T) -> PyResult<Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyTuple>> {
    let py: Python<'py> = faults.project_dir.py();
    let mut built: Vec<Bound<'py, PyAny>> = Vec::with_capacity(files.len());
    for scoped in files {
        let relative_path: Bound<'py, PyAny> =
            path_class(py)?.call1((path_object(py, tree, scoped.file.relative_path.clone())?,))?;
        let file_path: Bound<'py, PyAny> = faults.project_dir.div(&relative_path)?;
        let record: PyResult<Bound<'py, PyAny>> = match &scoped.file.outcome {
            FileOutcome::Parsed(file) => build(
                &FileContext {
                    tree,
                    relative_path: relative_path.clone(),
                    file_path,
                    scope: scoped.scope.as_ref(),
                },
                file,
            ),
            FileOutcome::Failed(failure) => Err(failure_error(py, failure.clone())),
            FileOutcome::Unreadable(failure) => Err(read_error(failure.clone(), &file_path)),
        };
        built.extend(faults.kept(&relative_path, record)?);
    }
    record_tuple(py, built)
}

fn discovered<'py>(
    faults: &FileFaults<'py>,
    kind: CollectionKind,
    isolate_kind: bool,
) -> PyResult<Bound<'py, PyTuple>> {
    let (tree, collection) = collection(
        &faults.project_dir,
        CollectionRequest { kind, isolate_kind },
    )?;
    let tree: &ProjectTree = &tree.get().inner;
    match collection.as_ref() {
        DeclarationCollection::Enums(files) => records(faults, tree, files_of(files), enum_file),
        DeclarationCollection::Constants(files) => {
            records(faults, tree, files_of(files), constant_file)
        }
        DeclarationCollection::ModelSchemas(files) => {
            records(faults, tree, files_of(files), schema_file)
        }
        DeclarationCollection::SqlFunctions(files) => {
            records(faults, tree, files_of(files), function_file)
        }
        DeclarationCollection::SqlHooks(files) => records(faults, tree, files_of(files), hook_file),
        DeclarationCollection::Audits(files) => records(faults, tree, files_of(files), audit_file),
        DeclarationCollection::Macros(files) => records(faults, tree, files_of(files), macro_file),
        DeclarationCollection::Seeds(files) => seed_records(faults, tree, files_of(files)),
    }
}

fn seed_records<'py, T>(
    faults: &FileFaults<'py>,
    tree: &ProjectTree,
    files: &[ScopedFile<T>],
) -> PyResult<Bound<'py, PyTuple>> {
    let py: Python<'py> = faults.project_dir.py();
    let seeds: Vec<Bound<'py, PyAny>> = files
        .iter()
        .map(|scoped| {
            let relative: Py<PyAny> = path_object(py, tree, scoped.file.relative_path.clone())?;
            new_record_with(
                py,
                RecordId::DiscoveredSeedFile,
                &[
                    ("file_path", faults.project_dir.div(relative.bind(py))?),
                    ("relative_path", path_class(py)?.call1((relative,))?),
                ],
            )
        })
        .collect::<PyResult<_>>()?;
    record_tuple(py, seeds)
}

fn faults<'py>(
    project_dir: &Bound<'py, PyAny>,
    on_fault: Option<Bound<'py, PyAny>>,
) -> FileFaults<'py> {
    FileFaults {
        project_dir: project_dir.clone(),
        on_fault: on_fault.filter(|on_fault| !on_fault.is_none()),
    }
}

/// Discover global and scoped enum declaration files.
#[pyfunction]
#[pyo3(signature = (*, project_dir, on_fault=None, isolate_declaration_kind=false))]
pub(crate) fn discover_enum_files<'py>(
    project_dir: &Bound<'py, PyAny>,
    on_fault: Option<Bound<'py, PyAny>>,
    isolate_declaration_kind: bool,
) -> PyResult<Bound<'py, PyTuple>> {
    discovered(
        &faults(project_dir, on_fault),
        CollectionKind::Enums,
        isolate_declaration_kind,
    )
}

/// Discover global and scoped constant declaration files.
#[pyfunction]
#[pyo3(signature = (*, project_dir, on_fault=None, isolate_declaration_kind=false))]
pub(crate) fn discover_constant_files<'py>(
    project_dir: &Bound<'py, PyAny>,
    on_fault: Option<Bound<'py, PyAny>>,
    isolate_declaration_kind: bool,
) -> PyResult<Bound<'py, PyTuple>> {
    discovered(
        &faults(project_dir, on_fault),
        CollectionKind::Constants,
        isolate_declaration_kind,
    )
}

/// Discover project-wide and scoped reusable model schemas.
#[pyfunction]
#[pyo3(signature = (*, project_dir, on_fault=None))]
pub(crate) fn discover_model_schema_files<'py>(
    project_dir: &Bound<'py, PyAny>,
    on_fault: Option<Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyTuple>> {
    discovered(
        &faults(project_dir, on_fault),
        CollectionKind::ModelSchemas,
        false,
    )
}

/// Discover SQL function files under functions/sql/.
#[pyfunction]
#[pyo3(signature = (*, project_dir, on_fault=None))]
pub(crate) fn discover_sql_function_files<'py>(
    project_dir: &Bound<'py, PyAny>,
    on_fault: Option<Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyTuple>> {
    discovered(
        &faults(project_dir, on_fault),
        CollectionKind::SqlFunctions,
        false,
    )
}

/// Discover named SQL lifecycle hooks in their project-wide and scoped roles.
#[pyfunction]
#[pyo3(signature = (*, project_dir, on_fault=None))]
pub(crate) fn discover_sql_hook_files<'py>(
    project_dir: &Bound<'py, PyAny>,
    on_fault: Option<Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyTuple>> {
    discovered(
        &faults(project_dir, on_fault),
        CollectionKind::SqlHooks,
        false,
    )
}

/// Discover generic and singular audit SQL files in their project-wide and scoped roles.
#[pyfunction]
#[pyo3(signature = (*, project_dir, on_fault=None))]
pub(crate) fn discover_audit_files<'py>(
    project_dir: &Bound<'py, PyAny>,
    on_fault: Option<Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyTuple>> {
    discovered(
        &faults(project_dir, on_fault),
        CollectionKind::Audits,
        false,
    )
}

/// Discover global and scoped project macro Python files.
#[pyfunction]
#[pyo3(signature = (*, project_dir, isolate_declaration_kind=false))]
pub(crate) fn discover_macro_files<'py>(
    project_dir: &Bound<'py, PyAny>,
    isolate_declaration_kind: bool,
) -> PyResult<Bound<'py, PyTuple>> {
    discovered(
        &faults(project_dir, None),
        CollectionKind::Macros,
        isolate_declaration_kind,
    )
}

/// Discover seed CSV files under seeds/.
#[pyfunction]
#[pyo3(signature = (*, project_dir))]
pub(crate) fn discover_seed_files<'py>(
    project_dir: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyTuple>> {
    discovered(&faults(project_dir, None), CollectionKind::Seeds, false)
}

/// Detect a project-level adapter.py without importing it.
#[pyfunction]
#[pyo3(signature = (*, project_dir))]
pub(crate) fn discover_adapter_file<'py>(
    project_dir: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = project_dir.py();
    let file_path: Bound<'py, PyAny> = project_dir.div("adapter.py")?;
    if !file_path.call_method0("is_file")?.is_truthy()? {
        return Ok(py.None().into_bound(py));
    }
    new_record_with(
        py,
        RecordId::DiscoveredAdapterFile,
        &[
            ("file_path", file_path),
            ("relative_path", path_class(py)?.call1(("adapter.py",))?),
        ],
    )
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(discover_enum_files, module)?)?;
    module.add_function(wrap_pyfunction!(discover_constant_files, module)?)?;
    module.add_function(wrap_pyfunction!(discover_model_schema_files, module)?)?;
    module.add_function(wrap_pyfunction!(discover_sql_function_files, module)?)?;
    module.add_function(wrap_pyfunction!(discover_sql_hook_files, module)?)?;
    module.add_function(wrap_pyfunction!(discover_audit_files, module)?)?;
    module.add_function(wrap_pyfunction!(discover_macro_files, module)?)?;
    module.add_function(wrap_pyfunction!(discover_seed_files, module)?)?;
    module.add_function(wrap_pyfunction!(discover_adapter_file, module)?)?;
    module.add_function(wrap_pyfunction!(retained_discovery_session, module)?)?;
    Ok(())
}
