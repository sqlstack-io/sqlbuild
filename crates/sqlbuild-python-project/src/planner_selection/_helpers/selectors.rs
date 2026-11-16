//! Planner selector parsing and resolution over native project graphs, raising `PlannerInputError`.

use crate::planner_selection::main::resolve_graph_keys::resolve_graph_keys;
use pyo3::prelude::{Bound, PyAny, PyResult, Python};
use pyo3::types::{PyBool, PyFrozenSet};
use sqlbuild_analysis::assembly::project::types::ObjectKey;
use sqlbuild_analysis::graph::errors::SelectorError;
use sqlbuild_analysis::graph::models::{ParsedSelector, ProjectGraph};

use crate::graph::main::project::graph_from_view_mappings::graph_from_view_mappings;
use crate::graph::models::GraphViewMappings;
use crate::graph::models::KeyCache;
use crate::graph::models::NativeProjectGraph;
use crate::planner_selection::models::Carrier;
use sqlbuild_python_base::boundary::models::KeywordArguments;
use sqlbuild_python_base::records::main::construct::enum_from_value::enum_from_value;
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use sqlbuild_python_base::records::types::RecordId;

pub(crate) const VIEW_OPTIONS: &[&str] = &[
    "all_keys",
    "upstream_deps",
    "downstream_deps",
    "tag_index",
    "path_index",
];

/// The parsed selector as Python's `ParsedSelector` or `PathSelector`.
pub(crate) fn parsed_selector<'py>(
    carrier: &Carrier<'py>,
    parsed: ParsedSelector,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = carrier.py();
    match parsed {
        ParsedSelector::Kind {
            kind,
            value,
            upstream,
            downstream,
        } => new_record_with(
            py,
            RecordId::ParsedSelector,
            &[
                ("kind", enum_from_value(py, RecordId::SelectorKind, kind)?),
                ("value", carrier.back(&value)?.into_any()),
                ("upstream", PyBool::new(py, upstream).to_owned().into_any()),
                (
                    "downstream",
                    PyBool::new(py, downstream).to_owned().into_any(),
                ),
            ],
        ),
        ParsedSelector::Path {
            start,
            end,
            upstream,
            downstream,
        } => new_record_with(
            py,
            RecordId::PathSelector,
            &[
                ("start_name", carrier.back(&start)?.into_any()),
                ("end_name", carrier.back(&end)?.into_any()),
                ("upstream", PyBool::new(py, upstream).to_owned().into_any()),
                (
                    "downstream",
                    PyBool::new(py, downstream).to_owned().into_any(),
                ),
            ],
        ),
    }
}

pub(crate) fn graph_names(graph: &ProjectGraph) -> Vec<String> {
    graph.names.iter().map(|(name, _)| name.clone()).collect()
}

pub(crate) fn selected<'py>(
    carrier: &Carrier<'py>,
    outcome: Result<Vec<ObjectKey>, SelectorError>,
) -> PyResult<Bound<'py, PyFrozenSet>> {
    let keys: Vec<ObjectKey> = outcome.map_err(|error| carrier.error(error))?;
    compiled_keys(carrier.py(), &keys)
}

fn compiled_keys<'py>(py: Python<'py>, keys: &[ObjectKey]) -> PyResult<Bound<'py, PyFrozenSet>> {
    PyFrozenSet::new(py, KeyCache::new(py)?.keys(keys)?)
}

pub(crate) fn resolved<'py>(
    graph: &ProjectGraph,
    (select, exclude): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
) -> PyResult<Bound<'py, PyFrozenSet>> {
    compiled_keys(select.py(), &resolve_graph_keys(graph, (select, exclude))?)
}

pub(crate) fn view_graph(mappings: GraphViewMappings<'_, '_>) -> PyResult<NativeProjectGraph> {
    graph_from_view_mappings(mappings)
}

pub(crate) fn arguments_graph(arguments: &KeywordArguments<'_>) -> PyResult<NativeProjectGraph> {
    view_graph(GraphViewMappings {
        all_keys: &arguments.value("all_keys")?,
        upstream: &arguments.value("upstream_deps")?,
        downstream: &arguments.value("downstream_deps")?,
        tag_index: &arguments.value("tag_index")?,
        path_index: &arguments.value("path_index")?,
    })
}
