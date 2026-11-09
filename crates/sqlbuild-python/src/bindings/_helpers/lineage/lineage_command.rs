//! `sqb lineage`: load or build the relation graph, select lineage, and print it.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use pyo3::exceptions::PyValueError;
use pyo3::prelude::{
    Bound, PyAny, PyAnyMethods, PyErr, PyModule, PyModuleMethods, PyResult, Python,
};
use pyo3::types::{PyDict, PyDictMethods, PyFrozenSet};
use pyo3::{pyfunction, wrap_pyfunction};
use sqlbuild_analysis::lineage_command::constants::{
    BOTH_DIRECTIONS, COLUMN_TARGET_SEPARATOR, DOWNSTREAM_DIRECTION, MODEL_RESOURCE_TYPE,
    RICH_STATUS_MODEL_THRESHOLD, SELECTOR_INTERSECTION_MARKER, UPSTREAM_DIRECTION,
};
use sqlbuild_analysis::lineage_command::errors::LineageError;
use sqlbuild_analysis::lineage_command::main::column_candidates::column_candidates;
use sqlbuild_analysis::lineage_command::main::lineage_path_nodes::lineage_path_nodes;
use sqlbuild_analysis::lineage_command::main::lineage_stdout::lineage_stdout;
use sqlbuild_analysis::lineage_command::main::normalize_lineage_target::normalize_lineage_target;
use sqlbuild_analysis::lineage_command::main::parse_lineage_depth::parse_lineage_depth;
use sqlbuild_analysis::lineage_command::main::render_column_trace::render_column_trace;
use sqlbuild_analysis::lineage_command::main::render_lineage_graph::render_lineage_graph;
use sqlbuild_analysis::lineage_command::main::select_anchored_lineage::select_anchored_lineage;
use sqlbuild_analysis::lineage_command::main::select_directional_lineage::select_directional_lineage;
use sqlbuild_analysis::lineage_command::main::select_target_lineage::select_target_lineage;
use sqlbuild_analysis::lineage_command::main::validate_lineage_request::validate_lineage_request;
use sqlbuild_analysis::lineage_command::models::{
    ColumnCandidates, ColumnTrace, LineageAnchors, LineageColumn, LineageColumnEdge, LineageIndex,
    LineageKey, LineageRendering, LineageRequestShape, SemanticUse,
};
use sqlbuild_core::cli_text::models::CliStyle;

use crate::bindings::_helpers::build_progress::transient_status::maybe_status;
use crate::bindings::_helpers::command_runtime::adapters::resolve_cli_adapter;
use crate::bindings::_helpers::compile_driver::discovery::discover_project_inputs;
use crate::bindings::_helpers::compile_driver::project::build_project_graph;
use crate::bindings::_helpers::contracts::spec_resolution::resolve_effective_adapter_name;
use crate::bindings::_helpers::lineage::lineage_cache::{
    lineage_fingerprint, read_lineage_cache, write_lineage_cache,
};
use crate::bindings::_helpers::lineage::lineage_index::{graph_index, index_views, lineage_keys};
use crate::bindings::_helpers::lineage::project_lineage::build_direct_semantic_uses;
use crate::bindings::_helpers::planner_selection::selectors::resolve_project_selector_tokens;
use crate::bindings::_helpers::planner_selection::selectors::resolve_project_selectors;
use crate::bindings::_helpers::planner_selection::selectors::select_project_graph;
use crate::bindings::_helpers::planner_selection::selectors::{
    match_project_selector, parse_project_selector,
};
use crate::bindings::_helpers::planner_selection::unit_tests::reject_unit_test_selectors;
use crate::bindings::_helpers::presentation_text::terminal::stdout_supports_color;
use crate::bindings::_helpers::records::construct::error_class;
use crate::bindings::_helpers::records::construct::record_class;
use crate::bindings::_helpers::records::fields::field as native_field;
use crate::bindings::_helpers::records::ids::{ErrorId, NativeAttr, RecordId};
use crate::bindings::_helpers::records::registry;

const UNKNOWN_SELECTOR_ERROR_CODE: &str = "S007";
const RICH_MODE: &str = "rich";
const ANALYSIS_HELP: &str = "enable SQL analysis or install SQLBuild with Polyglot support";
/// Selector views every planner selector function reads.
const GRAPH_VIEW_NAMES: [&str; 5] = [
    "all_keys",
    "upstream_deps",
    "downstream_deps",
    "tag_index",
    "path_index",
];
/// Selector kinds that match a set of resources rather than a named anchor.
const SET_SELECTOR_KINDS: [&str; 3] = ["tag", "path", "job"];

/// The relation graph lineage reads: a cached index, or a compiled graph with its adapter.
enum LineageSource<'py> {
    Cached(LineageIndex),
    Compiled {
        index: LineageIndex,
        graph: Bound<'py, PyAny>,
        adapter: Bound<'py, PyAny>,
    },
}

impl LineageSource<'_> {
    fn index(&self) -> &LineageIndex {
        match self {
            Self::Cached(index) | Self::Compiled { index, .. } => index,
        }
    }
}

fn keywords<'py>(
    py: Python<'py>,
    items: &[(&str, &Bound<'py, PyAny>)],
) -> PyResult<Bound<'py, PyDict>> {
    let dict: Bound<'py, PyDict> = PyDict::new(py);
    for (name, value) in items {
        dict.set_item(*name, value)?;
    }
    Ok(dict)
}

/// `CliUserError(message, code=..., help=...)`.
fn user_error(py: Python<'_>, error: LineageError) -> PyErr {
    let raised = (|| {
        let options: Bound<'_, PyDict> = PyDict::new(py);
        options.set_item("code", error.code)?;
        options.set_item("help", error.help)?;
        error_class(py, ErrorId::CliUser)?.call((error.message,), Some(&options))
    })();
    match raised {
        Ok(error) => PyErr::from_value(error),
        Err(error) => error,
    }
}

fn strings(value: &Bound<'_, PyAny>) -> PyResult<Vec<String>> {
    value.try_iter()?.map(|item| item?.extract()).collect()
}

fn name_lookup(index: &LineageIndex) -> HashMap<&str, &LineageKey> {
    index
        .all_keys
        .iter()
        .map(|(name, key)| (name.as_str(), key))
        .collect()
}

fn normalized_targets(
    py: Python<'_>,
    index: &LineageIndex,
    targets: &[String],
) -> PyResult<Vec<String>> {
    let names: HashMap<&str, &LineageKey> = name_lookup(index);
    let lookup = |name: &str| names.get(name).map(|key| (*key).clone());
    targets
        .iter()
        .map(|target| {
            normalize_lineage_target(target, &lookup).map_err(|error| user_error(py, error))
        })
        .collect()
}

/// `discover_project_inputs` with lineage's analysis override.
fn discover<'py>(
    py: Python<'py>,
    project_dir: &Bound<'py, PyAny>,
    analysis_override: Option<bool>,
) -> PyResult<Bound<'py, PyAny>> {
    let options: Bound<'py, PyDict> = PyDict::new(py);
    options.set_item("project_dir", project_dir)?;
    options.set_item("sql_analysis_enabled_override", analysis_override)?;
    options.set_item("extract_output_column_locations", false)?;
    discover_project_inputs(py, Some(options))
}

fn reject_unit_tests(request: &Bound<'_, PyAny>, discovered: &Bound<'_, PyAny>) -> PyResult<()> {
    reject_unit_test_selectors(
        &native_field(request, "select")?,
        &native_field(request, "exclude")?,
        discovered,
    )?;
    Ok(())
}

/// The cached relation index, or a freshly built graph that refreshes the cache.
fn prepare_source<'py>(
    py: Python<'py>,
    request: &Bound<'py, PyAny>,
    (project_dir, compiled): (&Bound<'py, PyAny>, bool),
) -> PyResult<LineageSource<'py>> {
    let directory: PathBuf = project_dir.extract()?;
    let fingerprint: Option<String> = if compiled {
        None
    } else {
        lineage_fingerprint(project_dir, &native_field(request, "cli_vars")?)?
    };
    if let Some(fingerprint) = &fingerprint
        && let Some(index) = read_lineage_cache(py, &directory, fingerprint)?
    {
        return Ok(LineageSource::Cached(index));
    }
    let discovered: Bound<'py, PyAny> =
        discover(py, project_dir, if compiled { None } else { Some(false) })?;
    reject_unit_tests(request, &discovered)?;
    let adapter_name: Bound<'py, PyAny> = resolve_effective_adapter_name(
        &native_field(&discovered, "project_config")?,
        &native_field(&discovered, "local_config")?,
    )?;
    let adapter: Bound<'py, PyAny> = resolve_cli_adapter(&adapter_name, project_dir)?;
    let no_validation: bool =
        native_field(request, "no_sql_validation")?.extract::<bool>()? || !compiled;
    let options: Bound<'py, PyDict> = keywords(
        py,
        &[
            ("discovered_inputs", &discovered),
            ("adapter", &adapter),
            ("cli_vars", &native_field(request, "cli_vars")?),
        ],
    )?;
    options.set_item("no_sql_validation", no_validation)?;
    options.set_item("skip_column_inference", !compiled)?;
    let graph: Bound<'py, PyAny> = build_project_graph(py, Some(options.clone()))?;
    let index: LineageIndex = graph_index(&graph)?;
    if let Some(fingerprint) = &fingerprint {
        write_lineage_cache(py, &directory, fingerprint, &index)?;
        return Ok(LineageSource::Cached(index));
    }
    Ok(LineageSource::Compiled {
        index,
        graph,
        adapter,
    })
}

fn compiled_graph_error(py: Python<'_>, message: &str, help: bool) -> PyErr {
    let error: LineageError = LineageError::new("C307", message);
    user_error(
        py,
        if help {
            error.with_help(ANALYSIS_HELP)
        } else {
            error
        },
    )
}

fn semantic_use(value: &Bound<'_, PyAny>) -> PyResult<SemanticUse> {
    let source: Bound<'_, PyAny> = native_field(value, "source")?;
    Ok(SemanticUse {
        consumer_model: native_field(value, "consumer_model")?.str()?.extract()?,
        context: native_field(value, "context")?.str()?.extract()?,
        expression: native_field(value, "expression_sql")?.str()?.extract()?,
        source: lineage_column(&source)?,
        confidence: native_field(value, "confidence")?
            .getattr("value")?
            .extract()?,
        line: native_field(value, "line")?.extract()?,
        column: native_field(value, "column")?.extract()?,
    })
}

/// Direct semantic column uses of the targeted or selected models.
fn semantic_uses(
    py: Python<'_>,
    request: &Bound<'_, PyAny>,
    source: &LineageSource<'_>,
) -> PyResult<Vec<SemanticUse>> {
    let LineageSource::Compiled {
        index,
        graph,
        adapter,
    } = source
    else {
        return Err(compiled_graph_error(
            py,
            "semantic uses require a compiled project graph",
            false,
        ));
    };
    let targets: Vec<String> = strings(&native_field(request, "targets")?)?;
    let model_names: Vec<String> = if targets.is_empty() {
        let keys: Vec<LineageKey> = lineage_keys(
            select_project_graph(
                graph,
                &native_field(request, "select")?,
                &native_field(request, "exclude")?,
            )?
            .as_any(),
        )?;
        keys.into_iter()
            .filter(|key| key.resource_type == MODEL_RESOURCE_TYPE)
            .map(|key| key.name)
            .collect()
    } else {
        normalized_targets(py, index, &targets)?
            .into_iter()
            .map(|target| {
                target
                    .split_once(COLUMN_TARGET_SEPARATOR)
                    .map_or(target.clone(), |(model, _)| model.to_owned())
            })
            .collect()
    };
    let uses: Bound<'_, PyAny> = build_direct_semantic_uses(
        &native_field(graph, "project")?,
        adapter
            .call_method0("sql_analysis_dialect")?
            .extract::<Option<String>>()?
            .as_deref(),
        PyFrozenSet::new(py, model_names)?.as_any(),
    )
    .and_then(|value| pyo3::IntoPyObjectExt::into_bound_py_any(value, py))?;
    uses.try_iter()?
        .map(|value| semantic_use(&value?))
        .collect()
}

fn lineage_column(column: &Bound<'_, PyAny>) -> PyResult<LineageColumn> {
    Ok(LineageColumn {
        resource_type: native_field(column, "resource_type")?.str()?.extract()?,
        resource_name: native_field(column, "resource_name")?.extract()?,
        column_name: native_field(column, "column_name")?.extract()?,
    })
}

fn column_edge(edge: &Bound<'_, PyAny>) -> PyResult<LineageColumnEdge> {
    Ok(LineageColumnEdge {
        source: lineage_column(&native_field(edge, "source")?)?,
        target: lineage_column(&native_field(edge, "target")?)?,
        transform: native_field(edge, "transform_kind")?.str()?.extract()?,
        confidence: native_field(edge, "confidence")?.str()?.extract()?,
    })
}

/// Edges reached depth first from one column, as Python's stack walk visits them.
fn traced_edges(
    lineage: &Bound<'_, PyAny>,
    (resource_name, column_name): (&str, &str),
    (direction, max_depth): (&str, Option<i64>),
) -> PyResult<Vec<LineageColumnEdge>> {
    let mut result: Vec<LineageColumnEdge> = Vec::new();
    if max_depth == Some(0) {
        return Ok(result);
    }
    let downstream: bool = direction == DOWNSTREAM_DIRECTION;
    let mut stack: Vec<(String, String, i64)> =
        vec![(resource_name.to_owned(), column_name.to_owned(), 0)];
    let mut visited: HashSet<(String, String)> = HashSet::new();
    while let Some((resource, column, depth)) = stack.pop() {
        if !visited.insert((resource.clone(), column.clone())) {
            continue;
        }
        if max_depth.is_some_and(|maximum| depth >= maximum) {
            continue;
        }
        let edges: Bound<'_, PyAny> = if downstream {
            let options: Bound<'_, PyDict> = PyDict::new(lineage.py());
            options.set_item("resource_name", &resource)?;
            options.set_item("column_name", &column)?;
            lineage.call_method("column_consumers", (), Some(&options))?
        } else {
            lineage.call_method1("edges_targeting", (&resource,))?
        };
        for edge in edges.try_iter()? {
            let edge: LineageColumnEdge = column_edge(&edge?)?;
            let next: &LineageColumn = if downstream {
                &edge.target
            } else {
                if edge.target.column_name != column {
                    continue;
                }
                &edge.source
            };
            stack.push((
                next.resource_name.clone(),
                next.column_name.clone(),
                depth + 1,
            ));
            result.push(edge);
        }
    }
    Ok(result)
}

/// Build column lineage under a stderr spinner for large rich-mode requests.
fn column_lineage<'py>(
    py: Python<'py>,
    (graph, mode): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
    candidates: &ColumnCandidates,
) -> PyResult<Bound<'py, PyAny>> {
    let count: usize = candidates.model_names.len();
    let rich: bool = mode.str()?.extract::<String>()? == RICH_MODE;
    let message: String = format!("Analyzing column lineage for {count} models...");
    maybe_status(
        py,
        (&message, rich && count >= RICH_STATUS_MODEL_THRESHOLD),
        || {
            registry::native(py, NativeAttr::BuildProjectColumnLineage)?.call1((
                native_field(graph, "project")?,
                py.None(),
                mode,
                PyFrozenSet::new(py, &candidates.model_names)?,
            ))
        },
    )
}

/// The column trace of a `model.column` target, or `None` when the target is not a model column.
fn column_trace(
    py: Python<'_>,
    source: &LineageSource<'_>,
    (target, mode): (&str, &Bound<'_, PyAny>),
    (direction, depth): (&str, Option<i64>),
) -> PyResult<Option<ColumnTrace>> {
    let Some((resource_name, column_name)) = target.rsplit_once(COLUMN_TARGET_SEPARATOR) else {
        return Ok(None);
    };
    let LineageSource::Compiled { index, graph, .. } = source else {
        return Err(compiled_graph_error(
            py,
            "column lineage requires a compiled project graph",
            true,
        ));
    };
    let Some(key) = name_lookup(index)
        .get(resource_name)
        .map(|key| (*key).clone())
    else {
        return Ok(None);
    };
    if key.resource_type != MODEL_RESOURCE_TYPE {
        return Ok(None);
    }
    if direction == BOTH_DIRECTIONS {
        return Err(user_error(
            py,
            LineageError::new(
                "C306",
                "column lineage supports --direction upstream or downstream, not both",
            ),
        ));
    }
    let candidates: ColumnCandidates = column_candidates(index, &key, (direction, depth));
    let lineage: Bound<'_, PyAny> = column_lineage(py, (graph, mode), &candidates)?;
    if lineage.is_none() {
        return Err(compiled_graph_error(
            py,
            "column lineage requires SQL analysis to be enabled and available",
            true,
        ));
    }
    Ok(Some(ColumnTrace {
        target: LineageColumn {
            resource_type: key.resource_type.clone(),
            resource_name: key.name.clone(),
            column_name: column_name.to_owned(),
        },
        trace: traced_edges(&lineage, (&key.name, column_name), (direction, depth))?,
        direction: direction.to_owned(),
        mode: native_field(mode, "value")?.extract()?,
        max_depth: depth,
        analyzed_model_count: candidates.model_names.len(),
        truncated: candidates.truncated,
    }))
}

/// Selector views of the source: the compiled graph's own dicts, or dicts built from the index.
fn selector_views<'py>(
    py: Python<'py>,
    source: &LineageSource<'py>,
) -> PyResult<Bound<'py, PyAny>> {
    match source {
        LineageSource::Compiled { graph, .. } => native_field(graph, "lineage_views"),
        LineageSource::Cached(index) => index_views(py, index),
    }
}

fn view_options<'py>(views: &Bound<'py, PyAny>, names: &[&str]) -> PyResult<Bound<'py, PyDict>> {
    let options: Bound<'py, PyDict> = PyDict::new(views.py());
    for name in names {
        options.set_item(*name, native_field(views, name)?)?;
    }
    Ok(options)
}

/// Depth anchors of selectors the shared resolver already validated.
fn selector_anchors(
    py: Python<'_>,
    (views, index): (&Bound<'_, PyAny>, &LineageIndex),
    select: &[String],
    require_clear_anchors: bool,
) -> PyResult<LineageAnchors> {
    let path_class: Bound<'_, PyAny> = record_class(py, RecordId::PathSelector)?;
    let names: HashMap<&str, &LineageKey> = name_lookup(index);
    let named = |name: String| -> PyResult<LineageKey> {
        names
            .get(name.as_str())
            .map(|key| (*key).clone())
            .ok_or_else(|| pyo3::exceptions::PyKeyError::new_err(name))
    };
    let mut anchors: LineageAnchors = LineageAnchors::default();
    for raw in select {
        for token in raw.split_whitespace() {
            if token.contains(SELECTOR_INTERSECTION_MARKER) && require_clear_anchors {
                return Err(user_error(
                    py,
                    LineageError::new(
                        "C308",
                        "--depth cannot be combined with comma-intersection selectors",
                    ),
                ));
            }
            for part in token.split(SELECTOR_INTERSECTION_MARKER) {
                let parsed: Bound<'_, PyAny> =
                    parse_project_selector(&pyo3::types::PyString::new(py, part))?;
                if parsed.is_instance(&path_class)? {
                    let start: LineageKey = named(native_field(&parsed, "start_name")?.extract()?)?;
                    let end: LineageKey = named(native_field(&parsed, "end_name")?.extract()?)?;
                    anchors
                        .retained
                        .extend(lineage_path_nodes(index, &start, &end));
                    anchors.upstream.push(start);
                    anchors.downstream.push(end);
                    continue;
                }
                let kind: String = native_field(&parsed, "kind")?.str()?.extract()?;
                let set_kind: bool = SET_SELECTOR_KINDS.contains(&kind.as_str());
                if set_kind && require_clear_anchors {
                    return Err(user_error(
                        py,
                        LineageError::new(
                            "C309",
                            "--depth requires name, source, seed, or path-between selectors",
                        ),
                    ));
                }
                let options: Bound<'_, PyDict> =
                    view_options(views, &["all_keys", "tag_index", "path_index"])?;
                let matched: Vec<LineageKey> = lineage_keys(
                    match_project_selector(
                        &parsed,
                        &options.as_any().get_item("all_keys")?,
                        &options.as_any().get_item("tag_index")?,
                        &options.as_any().get_item("path_index")?,
                    )?
                    .as_any(),
                )?;
                let upstream: bool = native_field(&parsed, "upstream")?.extract()?;
                let downstream: bool = native_field(&parsed, "downstream")?.extract()?;
                if set_kind || upstream {
                    anchors.upstream.extend(matched.iter().cloned());
                }
                if set_kind || downstream {
                    anchors.downstream.extend(matched.iter().cloned());
                }
                if !set_kind && !upstream && !downstream {
                    anchors.retained.extend(matched);
                }
            }
        }
    }
    Ok(anchors)
}

/// Lineage of `--select`/`--exclude`, expanded by direction or trimmed to depth.
fn selector_rendering(
    py: Python<'_>,
    request: &Bound<'_, PyAny>,
    source: &LineageSource<'_>,
    (depth, style): (Option<i64>, CliStyle),
) -> PyResult<LineageRendering> {
    let views: Bound<'_, PyAny> = selector_views(py, source)?;
    let select: Bound<'_, PyAny> = native_field(request, "select")?;
    let exclude: Bound<'_, PyAny> = native_field(request, "exclude")?;
    let options: Bound<'_, PyDict> = view_options(&views, &GRAPH_VIEW_NAMES)?;
    options.set_item("select", &select)?;
    options.set_item("exclude", &exclude)?;
    let selected: Vec<LineageKey> = lineage_keys(
        &resolve_project_selectors(py, Some(options.clone()))
            .and_then(|value| pyo3::IntoPyObjectExt::into_bound_py_any(value, py))?,
    )?;
    let index: &LineageIndex = source.index();
    let direction: Option<String> = native_field(request, "direction")?.extract()?;
    let output_format: String = native_field(request, "output_format")?.extract()?;
    let graph = if let Some(direction) = direction {
        let options: Bound<'_, PyDict> = view_options(&views, &GRAPH_VIEW_NAMES)?;
        options.set_item("selectors", &exclude)?;
        let excluded: Vec<LineageKey> = lineage_keys(
            &resolve_project_selector_tokens(py, Some(options.clone()))
                .and_then(|value| pyo3::IntoPyObjectExt::into_bound_py_any(value, py))?,
        )?;
        select_directional_lineage(index, (&selected, &excluded), (&direction, depth))
    } else {
        let anchors: LineageAnchors =
            selector_anchors(py, (&views, index), &strings(&select)?, depth.is_some())?;
        select_anchored_lineage(index, &selected, (&anchors, depth))
    };
    Ok(render_lineage_graph(&graph, &output_format, style))
}

/// Re-raise an unknown selector from a cached graph as the unit-test error it may hide.
fn selector_failure(
    py: Python<'_>,
    request: &Bound<'_, PyAny>,
    (source, project_dir): (&LineageSource<'_>, &Bound<'_, PyAny>),
    error: PyErr,
) -> PyErr {
    let check = || -> PyResult<()> {
        let planner_error: Bound<'_, PyAny> = error_class(py, ErrorId::PlannerInput)?;
        if !matches!(source, LineageSource::Cached(_))
            || !error.is_instance(py, &planner_error)
            || native_field(error.value(py), "code")?.extract::<String>()?
                != UNKNOWN_SELECTOR_ERROR_CODE
        {
            return Ok(());
        }
        reject_unit_tests(request, &discover(py, project_dir, Some(false))?)
    };
    match check() {
        Ok(()) => error,
        Err(raised) => raised,
    }
}

fn rendering(
    py: Python<'_>,
    request: &Bound<'_, PyAny>,
    (source, project_dir): (&LineageSource<'_>, &Bound<'_, PyAny>),
    (depth, style): (Option<i64>, CliStyle),
) -> PyResult<LineageRendering> {
    let targets: Vec<String> = strings(&native_field(request, "targets")?)?;
    let output_format: String = native_field(request, "output_format")?.extract()?;
    if targets.is_empty() {
        return selector_rendering(py, request, source, (depth, style))
            .map_err(|error| selector_failure(py, request, (source, project_dir), error));
    }
    let targets: Vec<String> = normalized_targets(py, source.index(), &targets)?;
    let direction: String = native_field(request, "direction")?
        .extract::<Option<String>>()?
        .unwrap_or_else(|| UPSTREAM_DIRECTION.to_owned());
    if let [target] = targets.as_slice()
        && let Some(trace) = column_trace(
            py,
            source,
            (target, &native_field(request, "lineage_mode")?),
            (&direction, depth),
        )?
    {
        return Ok(render_column_trace(&trace, &output_format, style));
    }
    let graph = select_target_lineage(source.index(), &targets, (&direction, depth))
        .map_err(|error| user_error(py, error))?;
    Ok(render_lineage_graph(&graph, &output_format, style))
}

fn request_shape(request: &Bound<'_, PyAny>) -> PyResult<LineageRequestShape> {
    Ok(LineageRequestShape {
        targets: strings(&native_field(request, "targets")?)?,
        select: strings(&native_field(request, "select")?)?,
        exclude: strings(&native_field(request, "exclude")?)?,
    })
}

/// Run one `sqb lineage` request and print its result.
#[pyfunction]
#[pyo3(signature = (*, request))]
pub(crate) fn run_lineage_command(py: Python<'_>, request: &Bound<'_, PyAny>) -> PyResult<i32> {
    let shape: LineageRequestShape = request_shape(request)?;
    validate_lineage_request(&shape).map_err(|error| user_error(py, error))?;
    let depth: Option<i64> =
        parse_lineage_depth(&native_field(request, "depth")?.extract::<String>()?)
            .map_err(|error| user_error(py, error))?;
    let project_dir: Bound<'_, PyAny> = match native_field(request, "project_dir")? {
        directory if !directory.is_none() => directory,
        _ => py.import("pathlib")?.getattr("Path")?.call_method0("cwd")?,
    };
    let include_uses: bool = native_field(request, "include_uses")?.extract()?;
    let compiled: bool = include_uses
        || shape
            .targets
            .iter()
            .any(|target| target.contains(COLUMN_TARGET_SEPARATOR));
    let source: LineageSource<'_> = prepare_source(py, request, (&project_dir, compiled))?;
    let uses: Option<Vec<SemanticUse>> = if include_uses {
        Some(semantic_uses(py, request, &source)?)
    } else {
        None
    };
    let style: CliStyle = CliStyle {
        use_color: stdout_supports_color(py)?,
    };
    let rendered: LineageRendering =
        rendering(py, request, (&source, &project_dir), (depth, style))?;
    let output: String = lineage_stdout(rendered, uses.as_deref())
        .map_err(|error| PyValueError::new_err(format!("{error:?}")))?;
    py.import("sys")?
        .getattr("stdout")?
        .call_method1("write", (output,))?;
    Ok(0)
}

/// The relation lineage cache key, or `None` when the inputs cannot be hashed.
#[pyfunction]
#[pyo3(signature = (*, project_dir, cli_vars))]
fn lineage_cache_fingerprint(
    project_dir: &Bound<'_, PyAny>,
    cli_vars: &Bound<'_, PyAny>,
) -> PyResult<Option<String>> {
    lineage_fingerprint(project_dir, cli_vars)
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(run_lineage_command, module)?)?;
    module.add_function(wrap_pyfunction!(lineage_cache_fingerprint, module)?)?;
    Ok(())
}
