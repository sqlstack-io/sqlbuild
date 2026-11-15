//! The explicit-reference facts of a compiled project, read from its Python views.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyBool, PyList, PyTuple};
use sqlbuild_analysis::explicit_references::models::{
    AuditGate, GraphResource, ModelReferences, NamedRelation, PythonHook, Relation, SourceRelation,
    SqlHook,
};
use sqlbuild_analysis::explicit_references::types::ResourceKey;
use sqlbuild_python_base::records::main::construct::record_class::record_class;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::main::fields::mapping_get::mapping_get;
use sqlbuild_python_base::records::types::RecordId;

const MODEL_HOOK_KEYS: [&str; 2] = ["pre_hooks", "post_hooks"];
const MIGRATE_FROM_KEY: &str = "migrate_from";
const OLD_NAME_VIEW_KEY: &str = "old_name_view";

/// The project facts the explicit-reference checks and execution-gate edges read.
pub(crate) struct ProjectFacts {
    /// Models, then sources, seeds and functions, with their dependencies.
    pub(crate) resources: Vec<GraphResource>,
    pub(crate) models: Vec<ModelReferences>,
    pub(crate) sources: Vec<SourceRelation>,
    pub(crate) seeds: Vec<NamedRelation>,
    pub(crate) audits: Vec<AuditGate>,
    pub(crate) hooks: Vec<PythonHook>,
}

/// The facts of `project`; `retains_old_names` is whether the project keeps old-name views.
pub(crate) fn project_facts(
    project: &Bound<'_, PyAny>,
    retains_old_names: bool,
) -> PyResult<ProjectFacts> {
    let py: Python<'_> = project.py();
    let entries: HookEntries<'_> = HookEntries::imported(py)?;
    let mut resources: Vec<GraphResource> = Vec::new();
    let mut models: Vec<ModelReferences> = Vec::new();
    for model in native_field(project, "models")?.try_iter()? {
        let model: Bound<'_, PyAny> = model?;
        resources.push(GraphResource {
            key: native_field(&model, "key_row")?.extract()?,
            deps: native_field(&model, "dep_rows")?.extract()?,
        });
        models.push(model_references(&model, &entries, retains_old_names)?);
    }
    let mut sources: Vec<SourceRelation> = Vec::new();
    for source in native_field(project, "sources")?.try_iter()? {
        let source: Bound<'_, PyAny> = source?;
        resources.push(graph_resource(&source)?);
        let entry: Bound<'_, PyAny> = native_field(&source, "source_entry")?;
        sources.push(SourceRelation {
            name: native_field(&source, "name")?.extract()?,
            table: native_field(&entry, "table")?.extract()?,
            schema: native_field(&entry, "schema")?.extract()?,
            database: native_field(&entry, "database")?.extract()?,
            expression: !native_field(&entry, "expression")?.is_none(),
        });
    }
    let mut seeds: Vec<NamedRelation> = Vec::new();
    for seed in native_field(project, "seeds")?.try_iter()? {
        let seed: Bound<'_, PyAny> = seed?;
        resources.push(graph_resource(&seed)?);
        let destination: Bound<'_, PyAny> = native_field(&seed, "destination")?;
        seeds.push(NamedRelation {
            name: native_field(&seed, "name")?.extract()?,
            relation: Relation {
                name: native_field(&destination, "name")?.extract()?,
                schema: native_field(&destination, "schema")?.extract()?,
                database: native_field(&destination, "database")?.extract()?,
            },
        });
    }
    for function in native_field(project, "functions")?.try_iter()? {
        resources.push(graph_resource(&function?)?);
    }
    let mut audits: Vec<AuditGate> = Vec::new();
    for audit in native_field(project, "audits")?.try_iter()? {
        audits.push(audit_gate(&audit?)?);
    }
    let mut hooks: Vec<PythonHook> = Vec::new();
    for hook in native_field(project, "hook_functions")?.try_iter()? {
        let hook: Bound<'_, PyAny> = hook?;
        hooks.push(PythonHook {
            name: native_field(&hook, "name")?.extract()?,
            path: native_field(&hook, "relative_path")?
                .call_method0("as_posix")?
                .extract()?,
            reads: ref_keys(&native_field(&hook, "reads")?)?,
        });
    }
    Ok(ProjectFacts {
        resources,
        models,
        sources,
        seeds,
        audits,
        hooks,
    })
}

/// `PythonHookEntry` and `SqlHookEntry`, which model hook entries are instances of.
struct HookEntries<'py> {
    python: Bound<'py, PyAny>,
    sql: Bound<'py, PyAny>,
}

impl<'py> HookEntries<'py> {
    fn imported(py: Python<'py>) -> PyResult<Self> {
        Ok(Self {
            python: record_class(py, RecordId::PythonHookEntry)?,
            sql: record_class(py, RecordId::SqlHookEntry)?,
        })
    }
}

fn model_references(
    model: &Bound<'_, PyAny>,
    entries: &HookEntries<'_>,
    retains_old_names: bool,
) -> PyResult<ModelReferences> {
    let values: Bound<'_, PyAny> = native_field(model, "config")?.getattr("values")?;
    let (database, schema, name): (Option<String>, Option<String>, String) =
        native_field(model, "destination_row")?.extract()?;
    let mut sql_hooks: Vec<SqlHook> = Vec::new();
    for hook_key in MODEL_HOOK_KEYS {
        for (index, entry) in hook_items(&mapping_get(&values, hook_key)?)?
            .into_iter()
            .enumerate()
        {
            if !entry.is_instance(&entries.sql)? || !native_field(&entry, "reads")?.is_truthy()? {
                continue;
            }
            let path: Bound<'_, PyAny> = native_field(&entry, "relative_path")?;
            sql_hooks.push(SqlHook {
                hook_key: hook_key.to_owned(),
                index,
                name: native_field(&entry, "name")?.extract()?,
                path: if path.is_none() {
                    None
                } else {
                    Some(path.str()?.extract()?)
                },
                reads: ref_keys(&native_field(&entry, "reads")?)?,
            });
        }
    }
    Ok(ModelReferences {
        name: native_field(model, "name")?.extract()?,
        path: native_field(model, "relative_path")?.str()?.extract()?,
        destination: Relation {
            name,
            schema,
            database,
        },
        python_hooks: python_hook_names(&values, entries)?,
        sql_hooks,
        migrate_from: migrate_from(&values, retains_old_names)?,
    })
}

/// `model_python_hook_names`: the model's Python hooks in lifecycle order, each once.
fn python_hook_names(
    values: &Bound<'_, PyAny>,
    entries: &HookEntries<'_>,
) -> PyResult<Vec<String>> {
    let mut names: Vec<String> = Vec::new();
    for hook_key in MODEL_HOOK_KEYS {
        let value: Bound<'_, PyAny> = mapping_get(values, hook_key)?;
        let items: Vec<Bound<'_, PyAny>> = if value.is_instance(&entries.python)? {
            vec![value]
        } else if is_sequence(&value) {
            value.try_iter()?.collect::<PyResult<_>>()?
        } else {
            Vec::new()
        };
        for item in items {
            if !item.is_instance(&entries.python)? {
                continue;
            }
            let name: String = native_field(&item, "name")?.extract()?;
            if !names.contains(&name) {
                names.push(name);
            }
        }
    }
    Ok(names)
}

/// The entries of one hook key: a list or tuple's items, or the value itself.
fn hook_items<'py>(value: &Bound<'py, PyAny>) -> PyResult<Vec<Bound<'py, PyAny>>> {
    if is_sequence(value) {
        return value.try_iter()?.collect();
    }
    Ok(vec![value.clone()])
}

fn is_sequence(value: &Bound<'_, PyAny>) -> bool {
    value.is_instance_of::<PyList>() || value.is_instance_of::<PyTuple>()
}

/// The `migrate_from` string when the model keeps an old-name view, as Python resolves it.
fn migrate_from(values: &Bound<'_, PyAny>, retains_old_names: bool) -> PyResult<Option<String>> {
    let raw: Bound<'_, PyAny> = mapping_get(values, MIGRATE_FROM_KEY)?;
    let Ok(text) = raw.extract::<String>() else {
        return Ok(None);
    };
    let retained: bool = if values.contains(OLD_NAME_VIEW_KEY)? {
        !values
            .get_item(OLD_NAME_VIEW_KEY)?
            .is(PyBool::new(values.py(), false))
    } else {
        retains_old_names
    };
    Ok(retained.then_some(text))
}

fn audit_gate(audit: &Bound<'_, PyAny>) -> PyResult<AuditGate> {
    let kind: Bound<'_, PyAny> = native_field(audit, "attached_target_kind")?;
    let name: Bound<'_, PyAny> = native_field(audit, "attached_target_name")?;
    let target: Option<ResourceKey> = if kind.is_none() || name.is_none() {
        None
    } else {
        Some((kind.str()?.extract()?, name.extract()?))
    };
    Ok(AuditGate {
        name: native_field(audit, "name")?.extract()?,
        path: native_field(audit, "audit_file")?
            .getattr("relative_path")?
            .str()?
            .extract()?,
        target,
        scope_deps: object_keys(&native_field(audit, "scope_deps")?)?,
    })
}

fn graph_resource(resource: &Bound<'_, PyAny>) -> PyResult<GraphResource> {
    Ok(GraphResource {
        key: object_key(&native_field(resource, "key")?)?,
        deps: object_keys(&native_field(resource, "deps")?)?,
    })
}

/// A `CompiledObjectKey` as its `(resource type, name)` pair.
fn object_key(key: &Bound<'_, PyAny>) -> PyResult<ResourceKey> {
    Ok((
        native_field(key, "resource_type")?.str()?.extract()?,
        native_field(key, "name")?.extract()?,
    ))
}

fn object_keys(keys: &Bound<'_, PyAny>) -> PyResult<Vec<ResourceKey>> {
    keys.try_iter()?.map(|key| object_key(&key?)).collect()
}

/// `SqlResourceRef`s as `(kind, name)` pairs.
pub(crate) fn ref_keys(refs: &Bound<'_, PyAny>) -> PyResult<Vec<ResourceKey>> {
    refs.try_iter()?
        .map(|reference| {
            let reference: Bound<'_, PyAny> = reference?;
            Ok((
                native_field(&reference, "kind")?.str()?.extract()?,
                native_field(&reference, "name")?.extract()?,
            ))
        })
        .collect()
}
