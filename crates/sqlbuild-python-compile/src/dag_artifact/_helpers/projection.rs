//! Dagster and Rivers asset and check plans over a DAG artifact, holding the exact translator inputs.

use std::collections::BTreeSet;

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods, PyList, PyMapping, PySet, PyTuple};
use pyo3::{IntoPyObject, pyfunction};
use sqlbuild_analysis::dag_artifact::main::dag_asset_plans::dag_asset_plans;
use sqlbuild_analysis::dag_artifact::main::dag_asset_spec_kinds::dag_asset_spec_kinds;
use sqlbuild_analysis::dag_artifact::main::dag_check_plans::dag_check_plans;
use sqlbuild_analysis::dag_artifact::main::dag_selected_asset_nodes::dag_selected_asset_nodes;
use sqlbuild_analysis::dag_artifact::models::{
    AssetNodePlan, AssetProjectionPolicy, AssetSpecKindPolicy, CheckKindFilter, CheckPlan,
    DagProjectionCheck, DagProjectionGraph, DagProjectionNode,
};
use sqlbuild_host::entries::models::ThirdPartyHost;

use crate::dag_artifact::_helpers::selection::selected_node_ids;
use sqlbuild_python_base::native_host::main::host_entry::host_entry;
use sqlbuild_python_base::records::main::fields::mapping_get::mapping_get;
use sqlbuild_python_base::records::main::fields::mapping_get_or::mapping_get_or;

const PROJECT_NAME_KEY: &str = "project_name";
const DEFAULT_MATERIALIZATION: &str = "table";

/// The artifact's nodes, edges and checks as Python objects alongside their projection facts.
struct ArtifactGraph<'py> {
    project_name: Bound<'py, PyAny>,
    nodes: Vec<Bound<'py, PyAny>>,
    checks: Vec<Bound<'py, PyAny>>,
    graph: DagProjectionGraph,
}

fn text(value: &Bound<'_, PyAny>) -> PyResult<String> {
    value.str()?.extract()
}

fn listed<'py>(value: &Bound<'py, PyAny>) -> PyResult<Vec<Bound<'py, PyAny>>> {
    value.try_iter()?.collect()
}

fn projection_node(node: &Bound<'_, PyAny>) -> PyResult<DagProjectionNode> {
    Ok(DagProjectionNode {
        id: text(&node.get_item("id")?)?,
        kind: text(&mapping_get(node, "kind")?)?,
    })
}

fn projection_check(check: &Bound<'_, PyAny>) -> PyResult<DagProjectionCheck> {
    let empty: Bound<'_, PyTuple> = PyTuple::empty(check.py());
    Ok(DagProjectionCheck {
        kind: text(&mapping_get(check, "kind")?)?,
        checked_asset_ids: listed(&mapping_get_or(check, "checked_asset_ids", empty)?)?
            .iter()
            .map(text)
            .collect::<PyResult<_>>()?,
    })
}

/// The artifact read the way the integrations index it: `nodes`, `edges` and `checks` by key.
fn artifact_graph<'py>(dag: &Bound<'py, PyAny>, with_checks: bool) -> PyResult<ArtifactGraph<'py>> {
    let nodes: Vec<Bound<'py, PyAny>> = listed(&dag.get_item("nodes")?)?;
    let checks: Vec<Bound<'py, PyAny>> = if with_checks {
        listed(&dag.get_item("checks")?)?
    } else {
        Vec::new()
    };
    let edges: Vec<(String, String)> = if with_checks {
        Vec::new()
    } else {
        listed(&dag.get_item("edges")?)?
            .iter()
            .map(|edge| {
                Ok((
                    text(&edge.get_item("from_id")?)?,
                    text(&edge.get_item("to_id")?)?,
                ))
            })
            .collect::<PyResult<_>>()?
    };
    Ok(ArtifactGraph {
        project_name: mapping_get(dag, PROJECT_NAME_KEY)?,
        graph: DagProjectionGraph {
            nodes: nodes.iter().map(projection_node).collect::<PyResult<_>>()?,
            edges,
            checks: checks
                .iter()
                .map(projection_check)
                .collect::<PyResult<_>>()?,
        },
        nodes,
        checks,
    })
}

/// `{**node, "project_name": project_name}`.
fn with_project_name<'py>(
    node: &Bound<'py, PyAny>,
    project_name: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let translated: Bound<'py, PyDict> = PyDict::new(node.py());
    translated.update(node.cast::<PyMapping>()?)?;
    translated.set_item(PROJECT_NAME_KEY, project_name)?;
    Ok(translated.into_any())
}

/// One `{**node, "project_name": ...}` per artifact node, in artifact order.
fn translated_nodes<'py>(artifact: &ArtifactGraph<'py>) -> PyResult<Vec<Bound<'py, PyAny>>> {
    artifact
        .nodes
        .iter()
        .map(|node| with_project_name(node, &artifact.project_name))
        .collect()
}

fn kind_set(py: Python<'_>, constant: ThirdPartyHost) -> PyResult<BTreeSet<String>> {
    listed(&host_entry(py, constant)?)?
        .iter()
        .map(text)
        .collect()
}

fn constant_text(py: Python<'_>, constant: ThirdPartyHost) -> PyResult<String> {
    text(&host_entry(py, constant)?)
}

fn picked<'py>(objects: &[Bound<'py, PyAny>], indexes: &[usize]) -> Vec<Bound<'py, PyAny>> {
    indexes
        .iter()
        .map(|index| objects[*index].clone())
        .collect()
}

/// `str(node.get("materialization_type") or "table")`.
fn materialization_type(node: &Bound<'_, PyAny>) -> PyResult<String> {
    let value: Bound<'_, PyAny> = mapping_get(node, "materialization_type")?;
    if value.is_truthy()? {
        text(&value)
    } else {
        Ok(DEFAULT_MATERIALIZATION.to_owned())
    }
}

/// Dagster asset specs to build: each translated asset node, its upstream nodes and its kinds.
#[pyfunction]
#[pyo3(signature = (dag, /))]
pub(crate) fn dagster_asset_plans<'py>(dag: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyList>> {
    let py: Python<'py> = dag.py();
    let artifact: ArtifactGraph<'py> = artifact_graph(dag, false)?;
    let upstream_nodes: Vec<Bound<'py, PyAny>> = translated_nodes(&artifact)?;
    let policy: AssetProjectionPolicy = AssetProjectionPolicy {
        asset_kinds: kind_set(py, ThirdPartyHost::DagsterAssetNodeKinds)?,
        upstream_asset_kinds_only: false,
    };
    let kinds: AssetSpecKindPolicy = AssetSpecKindPolicy {
        model_kind: constant_text(py, ThirdPartyHost::ModelNodeKind)?,
        view_materialization: constant_text(py, ThirdPartyHost::ViewMaterializationType)?,
        direct_kinds: kind_set(py, ThirdPartyHost::DagsterDirectKindNodeKinds)?,
    };
    let plans: Vec<Bound<'py, PyTuple>> = dag_asset_plans(&artifact.graph, &policy)
        .iter()
        .map(|plan: &AssetNodePlan| {
            let node: &Bound<'py, PyAny> = &artifact.nodes[plan.node];
            let spec_kinds: Vec<String> = dag_asset_spec_kinds(
                (
                    &artifact.graph.nodes[plan.node].kind,
                    &materialization_type(node)?,
                ),
                &kinds,
            );
            PyTuple::new(
                py,
                [
                    with_project_name(node, &artifact.project_name)?,
                    PyList::new(py, picked(&upstream_nodes, &plan.upstream))?.into_any(),
                    PySet::new(py, spec_kinds)?.into_any(),
                ],
            )
        })
        .collect::<PyResult<_>>()?;
    PyList::new(py, plans)
}

/// Rivers asset definitions to build: each translated asset node and its upstream asset nodes.
#[pyfunction]
#[pyo3(signature = (dag, /))]
pub(crate) fn rivers_asset_plans<'py>(dag: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyList>> {
    let py: Python<'py> = dag.py();
    let artifact: ArtifactGraph<'py> = artifact_graph(dag, false)?;
    let policy: AssetProjectionPolicy = AssetProjectionPolicy {
        asset_kinds: kind_set(py, ThirdPartyHost::RiversAssetNodeKinds)?,
        upstream_asset_kinds_only: true,
    };
    let plans: Vec<Bound<'py, PyTuple>> = dag_asset_plans(&artifact.graph, &policy)
        .iter()
        .map(|plan: &AssetNodePlan| {
            PyTuple::new(
                py,
                [
                    with_project_name(&artifact.nodes[plan.node], &artifact.project_name)?,
                    PyList::new(py, picked(&artifact.nodes, &plan.upstream))?.into_any(),
                ],
            )
        })
        .collect::<PyResult<_>>()?;
    PyList::new(py, plans)
}

/// `str(check.get("path"))`, or `None` without a path.
fn check_description<'py>(check: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let path: Bound<'py, PyAny> = mapping_get(check, "path")?;
    if path.is_none() {
        Ok(path)
    } else {
        Ok(path.str()?.into_any())
    }
}

/// Dagster check specs to build: check, kind exposed, translated checked nodes and description.
#[pyfunction]
#[pyo3(signature = (dag, /, *, include_scenarios, only_scenarios))]
pub(crate) fn dagster_check_plans<'py>(
    dag: &Bound<'py, PyAny>,
    include_scenarios: bool,
    only_scenarios: bool,
) -> PyResult<Bound<'py, PyList>> {
    let py: Python<'py> = dag.py();
    let artifact: ArtifactGraph<'py> = artifact_graph(dag, true)?;
    let checked_nodes: Vec<Bound<'py, PyAny>> = translated_nodes(&artifact)?;
    let scenario: BTreeSet<String> =
        BTreeSet::from([constant_text(py, ThirdPartyHost::ScenarioCheckKind)?]);
    let filter: CheckKindFilter = CheckKindFilter {
        excluded: if include_scenarios || only_scenarios {
            BTreeSet::new()
        } else {
            scenario.clone()
        },
        required: only_scenarios.then_some(scenario),
    };
    let plans: Vec<Bound<'py, PyTuple>> = dag_check_plans(&artifact.graph, &filter)
        .iter()
        .map(|plan: &CheckPlan| {
            let check: &Bound<'py, PyAny> = &artifact.checks[plan.check];
            PyTuple::new(
                py,
                [
                    check.clone(),
                    plan.included.into_pyobject(py)?.to_owned().into_any(),
                    PyList::new(py, picked(&checked_nodes, &plan.nodes))?.into_any(),
                    check_description(check)?,
                ],
            )
        })
        .collect::<PyResult<_>>()?;
    PyList::new(py, plans)
}

/// The translated nodes Dagster owns as assets among those `select` minus `exclude` matches.
#[pyfunction]
#[pyo3(signature = (dag, /, *, select, exclude = None))]
pub(crate) fn dagster_selected_asset_nodes<'py>(
    dag: &Bound<'py, PyAny>,
    select: &str,
    exclude: Option<&str>,
) -> PyResult<Bound<'py, PyList>> {
    let py: Python<'py> = dag.py();
    let selected: BTreeSet<String> = selected_node_ids(dag, (select, exclude))?;
    let nodes: Vec<Bound<'py, PyAny>> = listed(&mapping_get_or(dag, "nodes", PyTuple::empty(py))?)?;
    let project_name: Bound<'py, PyAny> = mapping_get(dag, PROJECT_NAME_KEY)?;
    let graph: DagProjectionGraph = DagProjectionGraph {
        nodes: nodes.iter().map(selection_node).collect::<PyResult<_>>()?,
        ..DagProjectionGraph::default()
    };
    let asset_kinds: BTreeSet<String> = kind_set(py, ThirdPartyHost::DagsterAssetNodeKinds)?;
    let translated: Vec<Bound<'py, PyAny>> =
        dag_selected_asset_nodes(&graph, &selected, &asset_kinds)
            .iter()
            .map(|index| with_project_name(&nodes[*index], &project_name))
            .collect::<PyResult<_>>()?;
    PyList::new(py, translated)
}

/// A node read the way selection reads it: `str(node.get("id"))` and `str(node.get("kind"))`.
fn selection_node(node: &Bound<'_, PyAny>) -> PyResult<DagProjectionNode> {
    Ok(DagProjectionNode {
        id: text(&mapping_get(node, "id")?)?,
        kind: text(&mapping_get(node, "kind")?)?,
    })
}
