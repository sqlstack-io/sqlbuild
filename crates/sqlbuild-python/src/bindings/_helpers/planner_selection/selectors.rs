//! Planner selector parsing and resolution over native project graphs, raising `PlannerInputError`.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyRef, PyResult, Python};
use pyo3::types::{PyBool, PyDict, PyFrozenSet, PySetMethods, PyString};
use sqlbuild_analysis::assembly::project::types::ObjectKey;
use sqlbuild_analysis::graph::errors::SelectorError;
use sqlbuild_analysis::graph::main::match_selector::match_selector;
use sqlbuild_analysis::graph::main::parse_selector::parse_selector;
use sqlbuild_analysis::graph::main::resolve_selector_tokens::resolve_selector_tokens;
use sqlbuild_analysis::graph::main::resolve_selectors::resolve_selectors;
use sqlbuild_analysis::graph::models::{ParsedSelector, ProjectGraph};

use crate::bindings::_helpers::boundary::python_calls::KeywordArguments;
use crate::bindings::_helpers::graph::algorithms::closure;
use crate::bindings::_helpers::graph::keys::KeyCache;
use crate::bindings::_helpers::graph::project_graph::{CompiledProjectGraph, NativeProjectGraph};
use crate::bindings::_helpers::graph::views::{GraphViewMappings, graph_from_view_mappings};
use crate::bindings::_helpers::planner_selection::carrier::{Carrier, py_strings};
use crate::bindings::_helpers::records::construct::{enum_from_value, new_record_with};
use crate::bindings::_helpers::records::fields::field as native_field;
use crate::bindings::_helpers::records::ids::RecordId;

const VIEW_OPTIONS: &[&str] = &[
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

fn graph_names(graph: &ProjectGraph) -> Vec<String> {
    graph.names.iter().map(|(name, _)| name.clone()).collect()
}

fn selected<'py>(
    carrier: &Carrier<'py>,
    outcome: Result<Vec<ObjectKey>, SelectorError>,
) -> PyResult<Bound<'py, PyFrozenSet>> {
    let keys: Vec<ObjectKey> = outcome.map_err(|error| carrier.error(error))?;
    compiled_keys(carrier.py(), &keys)
}

fn compiled_keys<'py>(py: Python<'py>, keys: &[ObjectKey]) -> PyResult<Bound<'py, PyFrozenSet>> {
    PyFrozenSet::new(py, KeyCache::new(py)?.keys(keys)?)
}

fn resolved<'py>(
    graph: &ProjectGraph,
    (select, exclude): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
) -> PyResult<Bound<'py, PyFrozenSet>> {
    compiled_keys(select.py(), &resolve_graph_keys(graph, (select, exclude))?)
}

/// Resolve graph keys for native planning without building a public frozenset.
pub(crate) fn resolve_graph_keys<'py>(
    graph: &ProjectGraph,
    (select, exclude): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
) -> PyResult<Vec<ObjectKey>> {
    let py: Python<'py> = select.py();
    let select: Vec<Bound<'py, PyString>> = py_strings(select)?;
    let exclude: Vec<Bound<'py, PyString>> = py_strings(exclude)?;
    let texts: Vec<Bound<'py, PyString>> = select.iter().chain(&exclude).cloned().collect();
    let carrier: Carrier<'py> = Carrier::new(py, &texts, || Ok(graph_names(graph)))?;
    resolve_selectors(
        graph,
        &carrier.send_all(&select)?,
        &carrier.send_all(&exclude)?,
    )
    .map_err(|error| carrier.error(error))
}

fn view_graph(mappings: GraphViewMappings<'_, '_>) -> PyResult<NativeProjectGraph> {
    graph_from_view_mappings(mappings)
}

/// Parse one selector token into Python's `ParsedSelector` or `PathSelector`.
pub(crate) fn parse_project_selector<'py>(
    raw: &Bound<'py, PyString>,
) -> PyResult<Bound<'py, PyAny>> {
    let carrier: Carrier<'py> = Carrier::single(raw)?;
    let parsed: ParsedSelector =
        parse_selector(&carrier.send(raw)?).map_err(|error| carrier.error(error))?;
    parsed_selector(&carrier, parsed)
}

/// `--select` minus `--exclude` on a native graph, adding the functions selected models need.
pub(crate) fn resolve_graph_selection<'py>(
    graph: PyRef<'py, NativeProjectGraph>,
    select: &Bound<'py, PyAny>,
    exclude: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyFrozenSet>> {
    resolved(graph.graph(), (select, exclude))
}

/// `--select` minus `--exclude` on a project graph, adding the functions selected models need.
pub(crate) fn select_project_graph<'py>(
    graph: &Bound<'py, PyAny>,
    select: &Bound<'py, PyAny>,
    exclude: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyFrozenSet>> {
    let graph: &Bound<'py, CompiledProjectGraph> = graph.cast()?;
    resolved(
        graph.get().native(graph.py()).get().graph(),
        (select, exclude),
    )
}

/// Model, source, seed and function selectors resolved against compiled graph mappings.
pub(crate) fn resolve_project_selectors<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyFrozenSet>> {
    let arguments: KeywordArguments<'py> = KeywordArguments::read(
        py,
        "resolve_project_selectors",
        options,
        (&[&["select", "exclude"], VIEW_OPTIONS].concat(), &[]),
    )?;
    let graph: NativeProjectGraph = arguments_graph(&arguments)?;
    resolved(
        graph.graph(),
        (&arguments.value("select")?, &arguments.value("exclude")?),
    )
}

/// The keys whitespace-separated selector tokens match, without required build resources.
pub(crate) fn resolve_project_selector_tokens<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyFrozenSet>> {
    let arguments: KeywordArguments<'py> = KeywordArguments::read(
        py,
        "resolve_project_selector_tokens",
        options,
        (&[&["selectors"], VIEW_OPTIONS].concat(), &[]),
    )?;
    let graph: NativeProjectGraph = arguments_graph(&arguments)?;
    let texts: Vec<Bound<'py, PyString>> = py_strings(&arguments.value("selectors")?)?;
    let carrier: Carrier<'py> = Carrier::new(py, &texts, || Ok(graph_names(graph.graph())))?;
    selected(
        &carrier,
        resolve_selector_tokens(graph.graph(), &carrier.send_all(&texts)?),
    )
}

fn arguments_graph(arguments: &KeywordArguments<'_>) -> PyResult<NativeProjectGraph> {
    view_graph(GraphViewMappings {
        all_keys: &arguments.value("all_keys")?,
        upstream: &arguments.value("upstream_deps")?,
        downstream: &arguments.value("downstream_deps")?,
        tag_index: &arguments.value("tag_index")?,
        path_index: &arguments.value("path_index")?,
    })
}

/// The keys one parsed selector matches before `+` graph expansion.
pub(crate) fn match_project_selector<'py>(
    parsed: &Bound<'py, PyAny>,
    all_keys: &Bound<'py, PyAny>,
    tag_index: &Bound<'py, PyAny>,
    path_index: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyFrozenSet>> {
    let py: Python<'py> = parsed.py();
    let empty: Bound<'py, PyAny> = PyDict::new(py).into_any();
    let graph: NativeProjectGraph = view_graph(GraphViewMappings {
        all_keys,
        upstream: &empty,
        downstream: &empty,
        tag_index,
        path_index,
    })?;
    let value: Bound<'py, PyString> = native_field(parsed, "value")?.downcast_into()?;
    let carrier: Carrier<'py> = Carrier::new(py, std::slice::from_ref(&value), || {
        Ok(graph_names(graph.graph()))
    })?;
    let kind: String = native_field(parsed, "kind")?.str()?.extract()?;
    selected(
        &carrier,
        match_selector(graph.graph(), &kind, &carrier.send(&value)?)
            .map(|keys| keys.into_iter().collect()),
    )
}

/// The upstream key closure of one compiled graph key.
pub(crate) fn expand_project_upstream_keys<'py>(
    key: Bound<'py, PyAny>,
    upstream_deps: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyFrozenSet>> {
    let visited = closure(vec![key], upstream_deps, (false, None))?;
    PyFrozenSet::new(upstream_deps.py(), visited.iter())
}
