//! Compile's explicit-reference checks over plain project facts, run natively.

use pyo3::prelude::{
    Bound, PyAny, PyAnyMethods, PyErr, PyModule, PyModuleMethods, PyResult, Python,
};
use std::collections::{BTreeMap, BTreeSet};

use pyo3::exceptions::{PyOSError, PyTypeError};
use pyo3::types::{PyDict, PyDictMethods, PySet, PyString, PyTuple};
use pyo3::{pyfunction, wrap_pyfunction};
use sqlbuild_analysis::explicit_references::errors::ReferenceFailure;
use sqlbuild_analysis::explicit_references::main::check_explicit_references::check_explicit_references;
use sqlbuild_analysis::explicit_references::main::execution_gate_edges::execution_gate_edges;
use sqlbuild_analysis::explicit_references::main::old_name_message::old_name_message;
use sqlbuild_analysis::explicit_references::main::relation_remedy::relation_remedy;
use sqlbuild_analysis::explicit_references::models::{
    ExecutionGateEdges, ExecutionGateRequest, GateEdge, HookEdge, HookType, LiteralSql,
    ReferenceReport, ReferenceRequest, SqlOwner,
};
use sqlbuild_analysis::explicit_references::types::ResourceKey;
use sqlbuild_config::project::main::explicit_references_help::explicit_references_help as references_help;
use sqlbuild_core::panics::main::native_failure::native_failure;
use sqlbuild_host::entries::models::UserCodeHost;
use sqlbuild_pysource::literal_sql::main::literal_sql_calls::literal_sql_calls;
use sqlbuild_pysource::literal_sql::models::FunctionSource;

use crate::bindings::_helpers::boundary::panics::compiler_error;
use crate::bindings::_helpers::boundary::python_calls::imported;
use crate::bindings::_helpers::graph::execution_gates::model_python_hook_names;
use crate::bindings::_helpers::native_host::host;
use crate::bindings::_helpers::records::construct::error_class;
use crate::bindings::_helpers::records::construct::{enum_from_value, new_record, record_class};
use crate::bindings::_helpers::records::fields::field as native_field;
use crate::bindings::_helpers::records::fields::mapping_items;
use crate::bindings::_helpers::records::ids::{ErrorId, RecordId};
use crate::bindings::_helpers::references::reference_facts::{ProjectFacts, project_facts};
use crate::bindings::types::CompilerDetach;

const EXPLICIT_REFERENCES_CONTEXT: &str = "native explicit-reference checks";
const REFERENCE_MODELS_MODULE: &str = "sqlbuild.compiler.references.models";
const BUILTINS_MODULE: &str = "builtins";
const VALUE_PLACEHOLDER: &str = "__sqlbuild_python_value_";
const HARD_CODED_ALLOWED: &str = "hard-coded relation names";
const NODE_OWNERS: [(&str, &str); 3] = [
    ("task", "task_functions"),
    ("asset", "asset_functions"),
    ("check", "check_functions"),
];
const NODE_KIND: &str = "node";
const HOOK_KIND: &str = "hook";
const LOADER_KIND: &str = "loader";
const MODEL_REF_KIND: &str = "model";
const SOURCE_REF_KIND: &str = "source";
const DIAGNOSTIC_PHASE: &str = "compile";
const DIAGNOSTIC_SEVERITY: &str = "error";

/// One Python function whose literal SQL the checks read, and the references it owns.
struct PythonSqlOwner<'py> {
    label: String,
    kind: &'static str,
    function: Bound<'py, PyAny>,
    relative_path: Bound<'py, PyAny>,
    own_refs: Vec<ResourceKey>,
    upstream_loader_by_source: Vec<(String, String)>,
}

impl<'py> PythonSqlOwner<'py> {
    fn new(label: String, kind: &'static str, node: &Bound<'py, PyAny>) -> PyResult<Self> {
        Ok(Self {
            label,
            kind,
            function: native_field(node, "function")?,
            relative_path: native_field(node, "relative_path")?,
            own_refs: Vec::new(),
            upstream_loader_by_source: Vec::new(),
        })
    }
}

/// Return P005, P007 and P008 errors and unmatched Python-SQL relations; raise P001.
#[pyfunction]
#[pyo3(signature = (*, project, discovered_inputs, enforce_explicit))]
pub(crate) fn explicit_reference_report<'py>(
    py: Python<'py>,
    project: &Bound<'py, PyAny>,
    discovered_inputs: &Bound<'py, PyAny>,
    enforce_explicit: bool,
) -> PyResult<Bound<'py, PyAny>> {
    let owners: Vec<PythonSqlOwner<'py>> = if enforce_explicit {
        python_sql_owners(project, discovered_inputs)?
    } else {
        Vec::new()
    };
    let path_order: Bound<'py, PyDict> = PyDict::new(py);
    let unique_paths: Bound<'py, PySet> =
        PySet::new(py, owners.iter().map(|owner| owner.relative_path.clone()))?;
    for (index, path) in imported(py, BUILTINS_MODULE, "sorted")?
        .call1((unique_paths,))?
        .try_iter()?
        .enumerate()
    {
        path_order.set_item(path?, index)?;
    }
    let retains_old_names: bool = !native_field(discovered_inputs, "project_config")?
        .getattr("migrations")?
        .getattr("old_name_views")?
        .is_none();
    let facts: ProjectFacts = project_facts(project, retains_old_names)?;
    let mut sql_owners: Vec<SqlOwner> = Vec::with_capacity(owners.len());
    for owner in &owners {
        let order: usize = path_order
            .as_any()
            .get_item(&owner.relative_path)?
            .extract()?;
        sql_owners.push(sql_owner(
            owner,
            order,
            literal_sql_calls_of(&owner.function)?,
        ));
    }
    let request = ReferenceRequest {
        enforce_explicit,
        dialect: native_field(project, "sql_analysis_dialect")?.extract()?,
        explicit_help: if enforce_explicit {
            references_help(HARD_CODED_ALLOWED)
        } else {
            String::new()
        },
        placeholder: VALUE_PLACEHOLDER.to_owned(),
        resources: facts.resources,
        models: facts.models,
        sources: facts.sources,
        seeds: facts.seeds,
        audits: facts.audits,
        hooks: facts.hooks,
        owners: sql_owners,
    };
    match py
        .compiler_detach(|| Ok(check_explicit_references(&request)))
        .map_err(compiler_error)?
    {
        Ok(report) => python_report(py, &report, &owners),
        Err(ReferenceFailure::CompileInput(message)) => Err(PyErr::from_value(
            error_class(py, ErrorId::CompileInput)?.call1((message,))?,
        )),
        Err(ReferenceFailure::UnknownDialect(message)) => Err(compiler_error(message)),
        Err(ReferenceFailure::Internal(reason)) => Err(compiler_error(native_failure(
            EXPLICIT_REFERENCES_CONTEXT,
            &reason,
        ))),
    }
}

/// Every task, asset, check, loader and hook function, as Python discovery lists them.
fn python_sql_owners<'py>(
    project: &Bound<'py, PyAny>,
    discovered_inputs: &Bound<'py, PyAny>,
) -> PyResult<Vec<PythonSqlOwner<'py>>> {
    let py: Python<'py> = project.py();
    let mut owners: Vec<PythonSqlOwner<'py>> = Vec::new();
    for (label, attribute) in NODE_OWNERS {
        for node in native_field(discovered_inputs, attribute)?.try_iter()? {
            let node: Bound<'py, PyAny> = node?;
            let name: String = native_field(&node, "name")?.extract()?;
            owners.push(PythonSqlOwner::new(
                format!("{label}:{name}"),
                NODE_KIND,
                &node,
            )?);
        }
    }
    owners.extend(loader_owners(project, discovered_inputs)?);
    let mut attached: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for model in native_field(project, "models")?.try_iter()? {
        let model: Bound<'py, PyAny> = model?;
        let model_name: String = native_field(&model, "name")?.extract()?;
        for hook_name in model_python_hook_names(py, &model)?.try_iter()? {
            attached
                .entry(hook_name?.extract()?)
                .or_default()
                .insert(model_name.clone());
        }
    }
    for hook in native_field(project, "hook_functions")?.try_iter()? {
        let hook: Bound<'py, PyAny> = hook?;
        let name: String = native_field(&hook, "name")?.extract()?;
        let mut owner = PythonSqlOwner::new(format!("hook:{name}"), HOOK_KIND, &hook)?;
        owner.own_refs = attached
            .get(&name)
            .into_iter()
            .flatten()
            .map(|model| (MODEL_REF_KIND.to_owned(), model.clone()))
            .collect();
        owners.push(owner);
    }
    Ok(owners)
}

/// Each loader function, owning its sources and naming the loaders it depends on.
fn loader_owners<'py>(
    project: &Bound<'py, PyAny>,
    discovered_inputs: &Bound<'py, PyAny>,
) -> PyResult<Vec<PythonSqlOwner<'py>>> {
    let py: Python<'py> = project.py();
    let mut sources_by_loader: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for source in native_field(project, "sources")?.try_iter()? {
        let source: Bound<'py, PyAny> = source?;
        let loader: Option<String> = native_field(&source, "source_entry")?
            .getattr("loader")?
            .extract()?;
        if let Some(loader) = loader {
            sources_by_loader
                .entry(loader)
                .or_default()
                .insert(native_field(&source, "name")?.extract()?);
        }
    }
    let read_definition = host::entry(py, UserCodeHost::ReadLoaderDefinition)?;
    let mut owners: Vec<PythonSqlOwner<'py>> = Vec::new();
    for loader in native_field(discovered_inputs, "loader_functions")?.try_iter()? {
        let loader: Bound<'py, PyAny> = loader?;
        let name: String = native_field(&loader, "name")?.extract()?;
        let mut upstream: Vec<(String, String)> = Vec::new();
        for dependency in native_field(&loader, "depends_on")?.try_iter()? {
            let dependency: Bound<'py, PyAny> = dependency?;
            if !dependency.is_callable() {
                continue;
            }
            let definition: Bound<'py, PyAny> = read_definition.call1((&dependency,))?;
            if definition.is_none() {
                continue;
            }
            let definition_name: String = native_field(&definition, "name")?.extract()?;
            let shown: String = match dependency.getattr_opt("__name__")? {
                Some(shown) => shown.extract()?,
                None => definition_name.clone(),
            };
            for source_name in sources_by_loader
                .get(&definition_name)
                .into_iter()
                .flatten()
            {
                upstream.retain(|(known, _)| known != source_name);
                upstream.push((source_name.clone(), shown.clone()));
            }
        }
        let mut owner = PythonSqlOwner::new(format!("loader:{name}"), LOADER_KIND, &loader)?;
        owner.own_refs = sources_by_loader
            .get(&name)
            .into_iter()
            .flatten()
            .map(|source| (SOURCE_REF_KIND.to_owned(), source.clone()))
            .collect();
        owner.upstream_loader_by_source = upstream;
        owners.push(owner);
    }
    Ok(owners)
}

/// The literal SQL calls of a function's dedented source, or none when its source is unreadable.
fn literal_sql_calls_of(function: &Bound<'_, PyAny>) -> PyResult<Vec<(String, String, usize)>> {
    let py: Python<'_> = function.py();
    let (lines, start_line): (Bound<'_, PyAny>, usize) =
        match imported(py, "inspect", "getsourcelines")?.call1((function,)) {
            Ok(found) => found.extract()?,
            Err(error)
                if error.is_instance_of::<PyOSError>(py)
                    || error.is_instance_of::<PyTypeError>(py) =>
            {
                return Ok(Vec::new());
            }
            Err(error) => return Err(error),
        };
    let joined: Bound<'_, PyAny> = PyString::new(py, "").call_method1("join", (lines,))?;
    let source: String = imported(py, "textwrap", "dedent")?
        .call1((joined,))?
        .extract()?;
    Ok(python_literal_sql_calls(
        py,
        &source,
        start_line,
        VALUE_PLACEHOLDER,
    ))
}

fn sql_owner(
    owner: &PythonSqlOwner<'_>,
    path_order: usize,
    literals: Vec<(String, String, usize)>,
) -> SqlOwner {
    SqlOwner {
        label: owner.label.clone(),
        kind: owner.kind.to_owned(),
        path_order,
        own_refs: owner.own_refs.clone(),
        upstream_loaders: owner.upstream_loader_by_source.clone(),
        literals: literals
            .into_iter()
            .map(|(method, sql, line)| LiteralSql { method, sql, line })
            .collect(),
    }
}

/// The report's diagnostics and unmatched relations as Python's compile models.
fn python_report<'py>(
    py: Python<'py>,
    report: &ReferenceReport,
    owners: &[PythonSqlOwner<'py>],
) -> PyResult<Bound<'py, PyAny>> {
    let reference_models = py.import(REFERENCE_MODELS_MODULE)?;
    let path_class: Bound<'py, PyAny> = py.import("pathlib")?.getattr("Path")?;
    let mut diagnostics: Vec<Bound<'py, PyAny>> = Vec::with_capacity(report.diagnostics.len());
    for row in &report.diagnostics {
        let fields: Bound<'py, PyDict> = PyDict::new(py);
        fields.set_item("phase", DIAGNOSTIC_PHASE)?;
        fields.set_item("severity", DIAGNOSTIC_SEVERITY)?;
        fields.set_item("code", row.code)?;
        fields.set_item("message", &row.message)?;
        fields.set_item("resource_type", row.resource.as_ref().map(|key| &key.0))?;
        fields.set_item("resource_name", row.resource.as_ref().map(|key| &key.1))?;
        match &row.path {
            Some(path) => fields.set_item("path", path_class.call1((path,))?)?,
            None => fields.set_item("path", py.None())?,
        }
        match row.owner {
            Some(owner) => {
                let location: Bound<'py, PyDict> = PyDict::new(py);
                location.set_item("path", &owners[owner].relative_path)?;
                location.set_item("line", row.line)?;
                location.set_item("column", 1)?;
                fields.set_item(
                    "location",
                    new_record(py, RecordId::SourceLocation, (), Some(&location))?,
                )?;
            }
            None => fields.set_item("location", py.None())?,
        }
        fields.set_item("help", &row.help)?;
        diagnostics.push(new_record(
            py,
            RecordId::CompilerDiagnostic,
            (),
            Some(&fields),
        )?);
    }
    let literal_relation: Bound<'py, PyAny> =
        record_class(reference_models.py(), RecordId::LiteralSqlRelation)?;
    let relation_name: Bound<'py, PyAny> = native_field(&reference_models, "RelationName")?;
    let mut unmatched: Vec<Bound<'py, PyAny>> = Vec::with_capacity(report.unmatched.len());
    for row in &report.unmatched {
        let owner: &PythonSqlOwner<'py> = &owners[row.owner];
        let relation: Bound<'py, PyDict> = PyDict::new(py);
        relation.set_item("name", &row.relation.name)?;
        relation.set_item("schema", &row.relation.schema)?;
        relation.set_item("database", &row.relation.database)?;
        let fields: Bound<'py, PyDict> = PyDict::new(py);
        fields.set_item("owner_label", &owner.label)?;
        fields.set_item(
            "owner_kind",
            enum_from_value(py, RecordId::HardCodedRelationOwnerKind, owner.kind)?,
        )?;
        fields.set_item("relative_path", &owner.relative_path)?;
        fields.set_item("line", row.line)?;
        fields.set_item("method", &row.method)?;
        fields.set_item("relation", relation_name.call((), Some(&relation))?)?;
        unmatched.push(literal_relation.call((), Some(&fields))?);
    }
    let fields: Bound<'py, PyDict> = PyDict::new(py);
    fields.set_item("diagnostics", PyTuple::new(py, diagnostics)?)?;
    fields.set_item("unmatched", PyTuple::new(py, unmatched)?)?;
    new_record(py, RecordId::PythonSqlReferenceReport, (), Some(&fields))
}

/// `(method, sql, line)` for each literal SQL call to the context in a dedented function source.
fn python_literal_sql_calls(
    py: Python<'_>,
    source: &str,
    start_line: usize,
    placeholder: &str,
) -> Vec<(String, String, usize)> {
    let version = py.version_info();
    literal_sql_calls(&FunctionSource {
        text: source,
        start_line,
        placeholder,
        python_version: (version.major, version.minor),
    })
    .into_iter()
    .map(|call| (call.method, call.sql, call.line))
    .collect()
}

/// How an owner of `owner_kind` should read `target` instead of naming its relation in SQL.
#[pyfunction]
#[pyo3(signature = (*, owner_kind, r#ref, upstream_loader_by_source = None))]
pub(crate) fn hard_coded_relation_remedy(
    owner_kind: &str,
    r#ref: &Bound<'_, PyAny>,
    upstream_loader_by_source: Option<&Bound<'_, PyAny>>,
) -> PyResult<String> {
    let target: ResourceKey = (
        native_field(r#ref, "kind")?.getattr("value")?.extract()?,
        native_field(r#ref, "name")?.extract()?,
    );
    let mut upstream_loaders: Vec<(String, String)> = Vec::new();
    if let Some(loaders) = upstream_loader_by_source.filter(|loaders| !loaders.is_none()) {
        for item in mapping_items(loaders)?.try_iter()? {
            upstream_loaders.push(item?.extract()?);
        }
    }
    Ok(relation_remedy(owner_kind, &target, &upstream_loaders))
}

/// The P008 message for SQL naming a relation kept only as a compatibility view.
#[pyfunction]
#[pyo3(signature = (*, owner_label, written, method, model_name))]
pub(crate) fn old_name_reference_message(
    owner_label: &str,
    written: &str,
    method: &str,
    model_name: &str,
) -> String {
    old_name_message(owner_label, written, method, model_name)
}

/// One attached-audit ordering edge: the audit, its target, and the gated and read resources.
pub(crate) struct GateRow {
    pub(crate) audit_name: String,
    pub(crate) target: ResourceKey,
    pub(crate) gated: ResourceKey,
    pub(crate) read: ResourceKey,
}

/// One hook-read ordering edge: the hook, the gated and read resources, and its type.
pub(crate) struct HookRow {
    pub(crate) hook_name: String,
    pub(crate) gated: ResourceKey,
    pub(crate) read: ResourceKey,
    /// The `HookReadType` value.
    pub(crate) hook_type: &'static str,
}

/// The attached-audit gate edges and hook-read edges of one project, in Python's order.
pub(crate) fn native_execution_gate_edges(
    project: &Bound<'_, PyAny>,
) -> PyResult<(Vec<GateRow>, Vec<HookRow>)> {
    let facts: ProjectFacts = project_facts(project, false)?;
    let edges: ExecutionGateEdges = execution_gate_edges(&ExecutionGateRequest {
        resources: facts.resources,
        models: facts.models,
        audits: facts.audits,
        hooks: facts.hooks,
    });
    Ok((
        edges.audit_gates.into_iter().map(gate_row).collect(),
        edges.hook_reads.into_iter().map(hook_row).collect(),
    ))
}

fn gate_row(edge: GateEdge) -> GateRow {
    GateRow {
        audit_name: edge.audit_name,
        target: edge.target,
        gated: edge.gated,
        read: edge.read,
    }
}

fn hook_row(edge: HookEdge) -> HookRow {
    let hook_type: &'static str = match edge.hook_type {
        HookType::Python => "Python",
        HookType::Sql => "SQL",
        HookType::InlineSql => "inline SQL",
    };
    HookRow {
        hook_name: edge.hook_name,
        gated: edge.gated,
        read: edge.read,
        hook_type,
    }
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(explicit_reference_report, module)?)?;
    module.add_function(wrap_pyfunction!(hard_coded_relation_remedy, module)?)?;
    module.add_function(wrap_pyfunction!(old_name_reference_message, module)?)?;
    Ok(())
}
