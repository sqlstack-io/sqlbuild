//! Display-ready Python plan entries and Python-aware selection warnings of a plan.

use sqlbuild_host::entries::models::UserCodeHost;

use sqlbuild_python_base::records::main::construct::enum_member::enum_member;
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use sqlbuild_python_base::records::main::construct::record_class::record_class;
use std::collections::HashMap;

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::{PyDict, PyString, PyTuple};

use sqlbuild_analysis::python_plan::main::ordered_python_names::ordered_python_names;
use sqlbuild_analysis::python_plan::main::upstream_closure::upstream_closure;

use sqlbuild_python_base::boundary::models::KeywordArguments;
use sqlbuild_python_base::native_host::main::host_entry::host_entry;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::main::fields::mapping_get::mapping_get;
use sqlbuild_python_base::records::types::RecordId;
use sqlbuild_python_project::graph::main::python_nodes::python_node_graph::python_node_graph;

const SKIPPED_UPSTREAM_CODE: &str = "P501";

/// The Python node kinds and plan phases entries are built from.
pub(crate) struct NodeKinds<'py> {
    task: Bound<'py, PyAny>,
    asset: Bound<'py, PyAny>,
}

impl<'py> NodeKinds<'py> {
    pub(crate) fn load(py: Python<'py>) -> PyResult<Self> {
        let kinds: Bound<'py, PyAny> = host_entry(py, UserCodeHost::PythonNodeKind)?;
        Ok(Self {
            task: native_field(&kinds, "TASK")?,
            asset: native_field(&kinds, "ASSET")?,
        })
    }

    /// Whether `kind` is a task or an asset.
    fn runs(&self, kind: &Bound<'py, PyAny>) -> PyResult<bool> {
        Ok(kind.eq(&self.task)? || kind.eq(&self.asset)?)
    }
}

/// `python_graph.upstream_deps` as names.
fn upstream_names(graph: &Bound<'_, PyAny>) -> PyResult<HashMap<String, Vec<String>>> {
    let mut upstream: HashMap<String, Vec<String>> = HashMap::new();
    for item in native_field(graph, "upstream_deps")?
        .call_method0("items")?
        .try_iter()?
    {
        let item: Bound<'_, PyAny> = item?;
        let names: Vec<String> = item
            .get_item(1)?
            .try_iter()?
            .map(|name| name?.extract())
            .collect::<PyResult<_>>()?;
        upstream.insert(item.get_item(0)?.extract()?, names);
    }
    Ok(upstream)
}

/// The discovered Python node graph.
pub(crate) fn node_graph<'py>(discovered: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    python_node_graph(discovered)
}

/// Return task/asset plan entries ordered by lifecycle dependency readiness.
#[pyfunction]
#[pyo3(signature = (**options))]
pub(crate) fn build_python_plan_entries<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyTuple>> {
    let arguments: KeywordArguments<'py> = KeywordArguments::read(
        py,
        "build_python_plan_entries",
        options,
        (
            &["lifecycle_plan", "python_graph"],
            &["previous_identities"],
        ),
    )?;
    let previous: Bound<'py, PyAny> = arguments.value("previous_identities")?;
    let previous: Bound<'py, PyAny> = if previous.is_none() {
        PyDict::new(py).into_any()
    } else {
        previous
    };
    plan_entries(
        &arguments.value("lifecycle_plan")?,
        (&arguments.value("python_graph")?, &NodeKinds::load(py)?),
        &previous,
    )
}

/// The entries of the lifecycle's ingress and then read-side tasks and assets.
pub(crate) fn plan_entries<'py>(
    lifecycle: &Bound<'py, PyAny>,
    (graph, kinds): (&Bound<'py, PyAny>, &NodeKinds<'py>),
    previous_identities: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyTuple>> {
    let py: Python<'py> = lifecycle.py();
    let upstream: HashMap<String, Vec<String>> = upstream_names(graph)?;
    let nodes: Bound<'py, PyAny> = native_field(graph, "nodes_by_name")?;
    let mut entries: Vec<Bound<'py, PyAny>> = Vec::new();
    for (attribute, phase) in [
        ("ingress_python_node_names", "PRE_SQL_INGRESS"),
        ("read_side_python_node_names", "READ_SIDE"),
    ] {
        let selected: Vec<String> = native_field(lifecycle, attribute)?
            .try_iter()?
            .map(|name| name?.extract())
            .collect::<PyResult<_>>()?;
        for name in ordered_python_names(&selected, &upstream) {
            let node: Bound<'py, PyAny> = nodes.get_item(name.as_str())?;
            let kind: Bound<'py, PyAny> = native_field(&node, "kind")?;
            if !kinds.runs(&kind)? {
                continue;
            }
            let identity: Bound<'py, PyAny> = native_field(&node, "identity")?;
            let previous: Bound<'py, PyAny> = previous_identity(&identity, previous_identities)?;
            let attribute_or_none = |value: &Bound<'py, PyAny>, name: &str| {
                if value.is_none() {
                    Ok(py.None().into_bound(py))
                } else {
                    native_field(value, name)
                }
            };
            entries.push(new_record_with(
                py,
                RecordId::PythonPlanEntry,
                &[
                    ("name", native_field(&node, "name")?),
                    ("kind", kind),
                    ("phase", enum_member(py, RecordId::PythonRunPhase, phase)?),
                    (
                        "identity_status",
                        identity_status(&identity, previous_identities)?,
                    ),
                    (
                        "current_definition_json",
                        attribute_or_none(&identity, "definition_json")?,
                    ),
                    (
                        "previous_definition_json",
                        attribute_or_none(&previous, "definition")?,
                    ),
                    (
                        "current_metadata_json",
                        attribute_or_none(&identity, "metadata_json")?,
                    ),
                    (
                        "previous_metadata_json",
                        attribute_or_none(&previous, "metadata_json")?,
                    ),
                    ("provider_usages", native_field(&node, "provider_usages")?),
                ],
            )?);
        }
    }
    PyTuple::new(py, entries)
}

/// `_previous_identity`: the stored fingerprint of a node's identity, or `None`.
fn previous_identity<'py>(
    identity: &Bound<'py, PyAny>,
    previous_identities: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    if identity.is_none() {
        return Ok(identity.clone());
    }
    let key: Bound<'py, PyTuple> = PyTuple::new(
        identity.py(),
        [
            native_field(identity, "node_type")?,
            native_field(identity, "node_name")?,
        ],
    )?;
    mapping_get(previous_identities, key)
}

/// `_identity_status`: whether a node's identity is unknown, new, unchanged or changed.
fn identity_status<'py>(
    identity: &Bound<'py, PyAny>,
    previous_identities: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let statuses: Bound<'py, PyAny> = record_class(identity.py(), RecordId::PythonIdentityStatus)?;
    if identity.is_none() {
        return native_field(&statuses, "UNKNOWN");
    }
    let previous: Bound<'py, PyAny> = previous_identity(identity, previous_identities)?;
    if previous.is_none() {
        return native_field(&statuses, "NEW");
    }
    if native_field(&previous, "version_hash")?.eq(native_field(identity, "version_hash")?)? {
        return native_field(&statuses, "UNCHANGED");
    }
    native_field(&statuses, "CHANGED")
}

/// Warnings for skipped task/asset deps of planned source loaders.
pub(crate) fn skipped_ingress_warnings<'py>(
    plan_output: &Bound<'py, PyAny>,
    run_selection: &Bound<'py, PyAny>,
    (graph, kinds): (&Bound<'py, PyAny>, &NodeKinds<'py>),
) -> PyResult<Vec<Bound<'py, PyAny>>> {
    let py: Python<'py> = plan_output.py();
    let mut loaders: Vec<String> = Vec::new();
    for entry in native_field(plan_output, "source_load_entries")?.try_iter()? {
        let loader: String = native_field(&entry?, "loader")?.extract()?;
        if !loaders.contains(&loader) {
            loaders.push(loader);
        }
    }
    loaders.sort();
    let nodes: Bound<'py, PyAny> = native_field(graph, "nodes_by_name")?;
    let selected: Bound<'py, PyAny> = native_field(run_selection, "python_node_names")?;
    let upstream: HashMap<String, Vec<String>> = upstream_names(graph)?;
    let severity: Bound<'py, PyAny> = enum_member(py, RecordId::WarningSeverity, "WARNING")?;
    let mut warnings: Vec<Bound<'py, PyAny>> = Vec::new();
    for loader in loaders {
        if !nodes.contains(loader.as_str())? {
            continue;
        }
        for upstream_name in upstream_closure(&loader, &upstream) {
            if selected.contains(upstream_name.as_str())? {
                continue;
            }
            let kind: Bound<'py, PyAny> =
                native_field(&nodes.get_item(upstream_name.as_str())?, "kind")?;
            if !kinds.runs(&kind)? {
                continue;
            }
            let message: String = format!(
                "Source loader '{loader}' has unselected upstream {} '{upstream_name}'. SQLBuild \
                 will not run it and cannot verify side effects or payload availability; use \
                 +source:{loader} to refresh upstream ingress dependencies.",
                native_field(&kind, "value")?.str()?
            );
            warnings.push(new_record_with(
                py,
                RecordId::PlanWarning,
                &[
                    ("model_name", py.None().into_bound(py)),
                    ("severity", severity.clone()),
                    ("message", PyString::new(py, &message).into_any()),
                    ("code", PyString::new(py, SKIPPED_UPSTREAM_CODE).into_any()),
                ],
            )?);
        }
    }
    Ok(warnings)
}
