//! Refactoring facts read from a compiled project, and the shared scope placement host.

use std::collections::{BTreeSet, HashMap};

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods, PyString, PyTuple, PyTupleMethods};
use sqlbuild_refactor::refactoring::models::{
    AuthoredFile, DeclarationPlacement, DependencyFacts, DestinationFacts, DiscoveredFile,
    ExpansionFacts, ExpansionSpan, LocationFacts, ManualLocation, ModelFacts, PlacedDeclaration,
    RefactorFacts, RefactorOperation, RefactorRequest, RelocatedDeclaration, ResourceColumnsFacts,
    SchemaColumnFacts, SqlFileRole, UnitTestFacts,
};
use sqlbuild_scopes::scope_index::models::{ResourceIdentity, ResourceKind};

use crate::bindings::_helpers::model_config::config_values::config_str;
use crate::bindings::_helpers::refactor::python_strings::find_python_string_locations;
use crate::bindings::_helpers::scopes::scope_lookup::build_scope_lookup;
use crate::bindings::_helpers::scopes::scope_reports::{scope_move_preview, scope_rebound_usages};
use crate::bindings::_helpers::scopes::scope_validation::relocated_declarations;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

const SQL_SUFFIX: &str = ".sql";
const GENERIC_DIALECT: &str = "generic";
const MATERIALIZED_KEY: &str = "materialized";
const MIGRATE_FROM_KEY: &str = "migrate_from";
const MODEL_KIND_PREFIX: &str = "model:";

fn posix(path: &Bound<'_, PyAny>) -> PyResult<String> {
    path.call_method0("as_posix")?.extract()
}

fn discovered_files(files: &Bound<'_, PyAny>) -> PyResult<Vec<DiscoveredFile>> {
    let mut discovered: Vec<DiscoveredFile> = Vec::new();
    for item in files.try_iter()? {
        let item: Bound<'_, PyAny> = item?;
        discovered.push(DiscoveredFile {
            path: posix(&native_field(&item, "relative_path")?)?,
            contents: native_field(&item, "contents")?.extract()?,
        });
    }
    Ok(discovered)
}

fn column_names(columns: &Bound<'_, PyAny>) -> PyResult<Vec<String>> {
    columns
        .try_iter()?
        .map(|column| native_field(&column?, "name")?.extract())
        .collect()
}

fn resource_columns(
    resources: &Bound<'_, PyAny>,
    entry: &str,
) -> PyResult<Vec<ResourceColumnsFacts>> {
    let mut facts: Vec<ResourceColumnsFacts> = Vec::new();
    for resource in resources.try_iter()? {
        let resource: Bound<'_, PyAny> = resource?;
        facts.push(ResourceColumnsFacts {
            name: native_field(&resource, "name")?.extract()?,
            columns: column_names(&native_field(&native_field(&resource, entry)?, "columns")?)?,
        });
    }
    Ok(facts)
}

fn spans(spans: &Bound<'_, PyAny>) -> PyResult<Vec<ExpansionSpan>> {
    let mut read: Vec<ExpansionSpan> = Vec::new();
    for span in spans.try_iter()? {
        let span: Bound<'_, PyAny> = span?;
        read.push(ExpansionSpan {
            source_start: native_field(&span, "source_start")?.extract()?,
            source_end: native_field(&span, "source_end")?.extract()?,
            output_start: native_field(&span, "output_start")?.extract()?,
            output_end: native_field(&span, "output_end")?.extract()?,
        });
    }
    Ok(read)
}

fn expansion_facts(expansion: &Bound<'_, PyAny>) -> PyResult<ExpansionFacts> {
    Ok(ExpansionFacts {
        expanded_sql: native_field(expansion, "expanded_sql")?.extract()?,
        passes: native_field(expansion, "passes")?
            .try_iter()?
            .map(|pass| spans(&pass?))
            .collect::<PyResult<_>>()?,
    })
}

fn schema_columns(entry: &Bound<'_, PyAny>) -> PyResult<Option<Vec<SchemaColumnFacts>>> {
    if entry.is_none() {
        return Ok(None);
    }
    let mut columns: Vec<SchemaColumnFacts> = Vec::new();
    for column in native_field(entry, "columns")?.try_iter()? {
        let column: Bound<'_, PyAny> = column?;
        let location: Bound<'_, PyAny> = native_field(&column, "location")?;
        columns.push(SchemaColumnFacts {
            name: native_field(&column, "name")?.extract()?,
            migrate_from: !native_field(&column, "migrate_from")?.is_none(),
            location: if location.is_none() {
                None
            } else {
                Some(LocationFacts {
                    path: posix(&native_field(&location, "path")?)?,
                    line: native_field(&location, "line")?.extract()?,
                    column: native_field(&location, "column")?.extract()?,
                })
            },
        });
    }
    Ok(Some(columns))
}

fn model(model: &Bound<'_, PyAny>, expansion: Option<&Bound<'_, PyAny>>) -> PyResult<ModelFacts> {
    let py: Python<'_> = model.py();
    let values: Bound<'_, PyDict> = PyDict::new(py);
    values.call_method1(
        "update",
        (native_field(&native_field(model, "config")?, "values")?,),
    )?;
    let materialized: Option<String> = config_str(&values, MATERIALIZED_KEY)?
        .map(|value| value.extract())
        .transpose()?;
    let migrate_from: Option<Bound<'_, PyAny>> = values.get_item(MIGRATE_FROM_KEY)?;
    let mut deps: Vec<DependencyFacts> = Vec::new();
    for dep in native_field(model, "deps")?.try_iter()? {
        let dep: Bound<'_, PyAny> = dep?;
        deps.push(DependencyFacts {
            resource_type: native_field(&dep, "resource_type")?.str()?.extract()?,
            name: native_field(&dep, "name")?.extract()?,
        });
    }
    let destination: Bound<'_, PyAny> = native_field(model, "destination")?;
    let inferred: Bound<'_, PyAny> = native_field(model, "inferred_columns")?;
    Ok(ModelFacts {
        name: native_field(model, "name")?.extract()?,
        path: posix(&native_field(model, "relative_path")?)?,
        deps,
        query_sql: native_field(model, "query_sql")?.extract()?,
        authored_query_sql: native_field(model, "authored_query_sql")?.extract()?,
        materialized,
        declares_migrate_from: migrate_from.is_some(),
        migrate_from_set: migrate_from.is_some_and(|value| !value.is_none()),
        destination: DestinationFacts {
            database: native_field(&destination, "database")?.extract()?,
            schema: native_field(&destination, "schema")?.extract()?,
            name: native_field(&destination, "name")?.extract()?,
        },
        inferred_columns: if inferred.is_none() {
            Vec::new()
        } else {
            column_names(&inferred)?
        },
        schema_columns: schema_columns(&native_field(model, "schema_entry")?)?,
        expansion: expansion.map(expansion_facts).transpose()?,
    })
}

/// Every compiled model's refactoring facts, or none without a project.
pub(crate) fn model_facts(
    refactor_project: &Bound<'_, PyAny>,
) -> PyResult<Option<Vec<ModelFacts>>> {
    if refactor_project.is_none() {
        return Ok(None);
    }
    let project: Bound<'_, PyAny> =
        native_field(&native_field(refactor_project, "graph")?, "project")?;
    let project_dir: Bound<'_, PyAny> = native_field(refactor_project, "project_dir")?;
    let mut expansions: HashMap<String, Bound<'_, PyAny>> = HashMap::new();
    let items: Vec<Bound<'_, PyAny>> = native_field(&project, "sql_expansions")?
        .call_method0("items")?
        .try_iter()?
        .collect::<PyResult<_>>()?;
    for item in items.iter().rev() {
        let (path, expansion): (Bound<'_, PyAny>, Bound<'_, PyAny>) = item.extract()?;
        expansions.insert(path.call_method0("resolve")?.str()?.extract()?, expansion);
    }
    let mut facts: Vec<ModelFacts> = Vec::new();
    for compiled in native_field(&project, "models")?.try_iter()? {
        let compiled: Bound<'_, PyAny> = compiled?;
        let resolved: String = project_dir
            .div(native_field(&compiled, "relative_path")?)?
            .call_method0("resolve")?
            .str()?
            .extract()?;
        facts.push(model(&compiled, expansions.get(&resolved))?);
    }
    Ok(Some(facts))
}

fn block_bodies(blocks: &Bound<'_, PyAny>) -> PyResult<Vec<String>> {
    blocks
        .try_iter()?
        .map(|block| native_field(&block?, "sql_body")?.extract())
        .collect()
}

/// One discovered file group: its role, attribute, body field and whether it has blocks.
struct FileGroup {
    role: SqlFileRole,
    attribute: &'static str,
    body: Option<&'static str>,
    blocks: bool,
}

fn authored_files(discovered: &Bound<'_, PyAny>) -> PyResult<Vec<AuthoredFile>> {
    let groups: [FileGroup; 6] = [
        FileGroup {
            role: SqlFileRole::Test,
            attribute: "test_files",
            body: None,
            blocks: true,
        },
        FileGroup {
            role: SqlFileRole::Scenario,
            attribute: "scenario_files",
            body: Some("sql_body"),
            blocks: false,
        },
        FileGroup {
            role: SqlFileRole::Audit,
            attribute: "audit_files",
            body: None,
            blocks: true,
        },
        FileGroup {
            role: SqlFileRole::Hook,
            attribute: "sql_hook_files",
            body: Some("sql_body"),
            blocks: false,
        },
        FileGroup {
            role: SqlFileRole::Function,
            attribute: "sql_function_files",
            body: Some("body_sql"),
            blocks: false,
        },
        FileGroup {
            role: SqlFileRole::Schema,
            attribute: "model_schema_files",
            body: None,
            blocks: false,
        },
    ];
    let mut files: Vec<AuthoredFile> = Vec::new();
    for FileGroup {
        role,
        attribute,
        body,
        blocks,
    } in groups
    {
        for item in native_field(discovered, attribute)?.try_iter()? {
            let item: Bound<'_, PyAny> = item?;
            let texts: Vec<String> = match (body, blocks) {
                (Some(body), _) => vec![native_field(&item, body)?.extract()?],
                (None, true) => block_bodies(&native_field(&item, "blocks")?)?,
                (None, false) => Vec::new(),
            };
            files.push(AuthoredFile {
                role,
                path: posix(&native_field(&item, "relative_path")?)?,
                contents: native_field(&item, "contents")?.extract()?,
                texts,
            });
        }
    }
    Ok(files)
}

fn python_locations(
    project: &Bound<'_, PyAny>,
    request: &RefactorRequest,
) -> PyResult<Vec<ManualLocation>> {
    let py: Python<'_> = project.py();
    let old: &str = &request.model_name;
    let options: Bound<'_, PyDict> = PyDict::new(py);
    options.set_item("project_dir", native_field(project, "project_dir")?)?;
    options.set_item("discovered", native_field(project, "discovered")?)?;
    if request.operation == RefactorOperation::RenameColumn {
        let column: &str = request.column_name.as_deref().unwrap_or_default();
        options.set_item("names", PyTuple::new(py, [column])?)?;
        options.set_item(
            "reason",
            format!("Python SQL may read column {column} of {old}; check it by hand"),
        )?;
        options.set_item("context", old)?;
    } else {
        options.set_item("names", PyTuple::new(py, [old])?)?;
        options.set_item(
            "reason",
            format!("Python code names model {old}; update it by hand"),
        )?;
    }
    let mut locations: Vec<ManualLocation> = Vec::new();
    for location in find_python_string_locations(Some(&options))?.try_iter()? {
        let location: Bound<'_, PyAny> = location?;
        locations.push(ManualLocation {
            path: native_field(&location, "path")?.extract()?,
            line: native_field(&location, "line")?.extract()?,
            column: native_field(&location, "column")?.extract()?,
            reason: native_field(&location, "reason")?.extract()?,
        });
    }
    Ok(locations)
}

/// Whether `request` moves a model without renaming it: a folder, or a file of the same name.
fn move_keeps_name(request: &RefactorRequest) -> bool {
    if request.operation != RefactorOperation::MoveModel {
        return false;
    }
    let destination: &str = request.destination.as_deref().unwrap_or_default();
    match destination.strip_suffix(SQL_SUFFIX) {
        Some(path) => path.rsplit('/').next() == Some(request.model_name.as_str()),
        None => true,
    }
}

/// The facts one refactoring plan of `requests` reads.
pub(crate) fn refactor_facts(
    project: &Bound<'_, PyAny>,
    requests: &[RefactorRequest],
) -> PyResult<RefactorFacts> {
    let mut locations: Vec<ManualLocation> = Vec::new();
    for request in requests {
        if !move_keeps_name(request) {
            locations.extend(python_locations(project, request)?);
        }
    }
    let compiled: Bound<'_, PyAny> = native_field(&native_field(project, "graph")?, "project")?;
    let discovered: Bound<'_, PyAny> = native_field(project, "discovered")?;
    let dialect: Option<String> = native_field(&compiled, "sql_analysis_dialect")?.extract()?;
    let mut yaml_files: Vec<DiscoveredFile> =
        discovered_files(&native_field(&discovered, "source_files")?)?;
    yaml_files.extend(discovered_files(&native_field(
        &discovered,
        "schema_files",
    )?)?);
    Ok(RefactorFacts {
        project_dir: native_field(project, "project_dir")?.str()?.extract()?,
        dialect: dialect
            .filter(|dialect| !dialect.is_empty())
            .unwrap_or_else(|| GENERIC_DIALECT.to_owned()),
        models: model_facts(project)?.unwrap_or_default(),
        sources: resource_columns(&native_field(&compiled, "sources")?, "source_entry")?,
        seeds: resource_columns(&native_field(&compiled, "seeds")?, "schema_entry")?,
        model_files: discovered_files(&native_field(&discovered, "model_files")?)?,
        authored_files: authored_files(&discovered)?,
        yaml_files,
        unit_tests: unit_tests(&compiled)?,
        python_locations: locations,
    })
}

/// Every compiled SQL unit test with its file, header name and target models.
fn unit_tests(compiled: &Bound<'_, PyAny>) -> PyResult<Vec<UnitTestFacts>> {
    let mut tests: Vec<UnitTestFacts> = Vec::new();
    for test in native_field(compiled, "sql_tests")?.try_iter()? {
        let test: Bound<'_, PyAny> = test?;
        tests.push(UnitTestFacts {
            path: posix(&native_field(
                &native_field(&test, "test_file")?,
                "relative_path",
            )?)?,
            name: native_field(&native_field(&test, "test_block")?, "name")?.extract()?,
            target_model_names: native_field(&test, "target_model_names")?.extract()?,
        });
    }
    Ok(tests)
}

fn identity_key(identity: &Bound<'_, PyAny>) -> PyResult<String> {
    identity.repr()?.extract()
}

/// Scoped declarations the test file at `path` uses that resolve differently from `destination`.
pub(crate) fn unit_test_rebindings(
    project: &Bound<'_, PyAny>,
    paths: (&str, &str),
) -> PyResult<Option<Vec<String>>> {
    let py: Python<'_> = project.py();
    let index: Bound<'_, PyAny> = native_field(
        &native_field(&native_field(project, "graph")?, "project")?,
        "scope_index",
    )?;
    let lookup: Bound<'_, PyAny> = build_scope_lookup(py, &index)?.into_bound(py);
    scope_rebound_usages(py, &lookup, paths)
}

/// Where scope placement puts declarations once every `(model, destination)` moves together.
pub(crate) fn declaration_placement(
    project: &Bound<'_, PyAny>,
    moves: &[(String, String)],
    vacated: &BTreeSet<String>,
) -> PyResult<DeclarationPlacement> {
    let py: Python<'_> = project.py();
    let index: Bound<'_, PyAny> = native_field(
        &native_field(&native_field(project, "graph")?, "project")?,
        "scope_index",
    )?;
    let lookup: Bound<'_, PyAny> = build_scope_lookup(py, &index)?.into_bound(py);
    let mut messages: Vec<String> = Vec::new();
    let mut previewed: bool = true;
    for (model, destination) in moves.iter().filter(|(_, path)| !vacated.contains(path)) {
        let (preview, diagnostics) = scope_move_preview(
            py,
            &lookup,
            &PyString::new(py, &format!("{MODEL_KIND_PREFIX}{model}")).into_any(),
            &PyString::new(py, destination).into_any(),
        )?;
        for item in diagnostics.into_bound(py).try_iter()? {
            messages.push(native_field(&item?, "message")?.extract()?);
        }
        if preview.is_none() {
            previewed = false;
            break;
        }
    }
    let relocated: Option<Vec<RelocatedDeclaration>> = if previewed {
        let resources: Vec<(ResourceIdentity, String)> = moves
            .iter()
            .map(|(model, destination)| {
                let resource: ResourceIdentity = ResourceIdentity {
                    kind: ResourceKind::Model,
                    name: model.clone(),
                };
                (resource, destination.clone())
            })
            .collect();
        let records = relocated_declarations(&index, &resources)?;
        if let Some((records, flags)) = records {
            let mut grouping: HashMap<String, bool> = HashMap::new();
            for (record, requires_group) in records.iter().zip(flags) {
                grouping.insert(
                    identity_key(&native_field(&record, "identity")?)?,
                    requires_group,
                );
            }
            let sort: Bound<'_, PyDict> = PyDict::new(py);
            sort.set_item(
                "key",
                py.import("operator")?
                    .getattr("attrgetter")?
                    .call1(("identity",))?,
            )?;
            let mut relocated: Vec<RelocatedDeclaration> = Vec::new();
            for record in py
                .import("builtins")?
                .getattr("sorted")?
                .call((records,), Some(&sort))?
                .try_iter()?
            {
                let record: Bound<'_, PyAny> = record?;
                let key: String = identity_key(&native_field(&record, "identity")?)?;
                relocated.push(RelocatedDeclaration {
                    requires_group: grouping.get(&key).copied().unwrap_or(false),
                    key,
                    path: native_field(&record, "path")?.extract()?,
                });
            }
            Some(relocated)
        } else {
            None
        }
    } else {
        None
    };
    let mut declarations: Vec<PlacedDeclaration> = Vec::new();
    for record in native_field(&index, "declarations")?.try_iter()? {
        let record: Bound<'_, PyAny> = record?;
        let identity: Bound<'_, PyAny> = native_field(&record, "identity")?;
        let kind: String = native_field(&native_field(&identity, "kind")?, "value")?.extract()?;
        let name: String = native_field(&identity, "name")?.extract()?;
        declarations.push(PlacedDeclaration {
            key: identity_key(&identity)?,
            label: format!("{kind}:{name}"),
            path: native_field(&record, "path")?.extract()?,
            line: native_field(&record, "line")?.extract()?,
            column: native_field(&record, "column")?.extract()?,
        });
    }
    Ok(DeclarationPlacement {
        relocated,
        diagnostics: messages,
        declarations,
    })
}
