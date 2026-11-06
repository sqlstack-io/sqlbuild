//! Cursor MIN/MAX inspection of selected incremental models and their watermark inputs.

use crate::bindings::_helpers::boundary::python_calls::{call_with, imported, keywords};
use crate::bindings::_helpers::compile_attachment::cursor_roles::resolve_cursor_input_roles;
use crate::bindings::_helpers::compiled_views::model_reads;
use crate::bindings::_helpers::compiled_views::project_reads;
use crate::bindings::_helpers::compiled_views::resource_reads;
use crate::bindings::_helpers::compiled_views::source_reads;
use crate::bindings::_helpers::cursor::intervals::partition;
use crate::bindings::_helpers::cursor::scalars;
use crate::bindings::_helpers::cursor::scalars::compare_values;
use crate::bindings::_helpers::cursor::scalars::render_value;
use crate::bindings::_helpers::cursor::scalars::to_inclusive;
use crate::bindings::_helpers::model_config::typed_config::{config_cursor_bound, model_values};
use crate::bindings::_helpers::native_host::host;
use crate::bindings::_helpers::planner_phases::objects::object;
use crate::bindings::_helpers::planner_phases::objects::{
    by_name, config_str, model_key, native_member, optional_text, planner_error, planner_model,
    text, tuple,
};
use crate::bindings::_helpers::planner_resolution::cursor_policies::resolve_start_cursor_config;
use crate::bindings::_helpers::planner_resolution::cursor_windows::normalize_cursor_snapshot_grain;
use crate::bindings::_helpers::planner_resolution::cursor_windows::resolve_effective_timestamp_grain;
use crate::bindings::_helpers::planner_resolution::maximum_safety::maximum_allowed_start;
use crate::bindings::_helpers::planner_strategy::lineage::resolve_lineage_reference;
use crate::bindings::_helpers::planner_warehouse::constants::{
    CURSOR_ALGEBRA_ERROR, CURSOR_TYPE_INTEGER, CURSOR_TYPE_TIMESTAMP, DATE_VALUE, LOG_DEBUG_EVENT,
    PLANNER_LOGGER, QUERY_RECORD, RECORD_QUERY, RUN_BOUNDED, RUN_RECORDED_QUERY,
};
use crate::bindings::_helpers::planner_warehouse::snapshot::{
    SnapshotRequest, inspection_concurrency,
};
use crate::bindings::_helpers::planner_warehouse::watermarks::validate_watermark_column;
use crate::bindings::_helpers::records::construct::record_class;
use crate::bindings::_helpers::records::fields::field as native_field;
use crate::bindings::_helpers::records::fields::mapping_items;
use crate::bindings::_helpers::records::ids::RecordId;
use crate::bindings::_helpers::references::source_relations::render_source_relation;
use pyo3::IntoPyObject;
use pyo3::exceptions::PyException;
use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyErr, PyResult, Python};
use pyo3::types::{PyCFunction, PyDict, PyDictMethods, PyFloat, PyTuple};
use sqlbuild_analysis::planner_warehouse::main::cursor_bounds::cursor_bounds;
use sqlbuild_analysis::planner_warehouse::main::cursor_query_groups::cursor_query_groups;
use sqlbuild_analysis::planner_warehouse::main::cursor_query_identity::cursor_query_identity;
use sqlbuild_analysis::planner_warehouse::main::cursor_query_sql::cursor_query_sql;
use sqlbuild_analysis::planner_warehouse::models::{CursorTagRequest, PhysicalCursorQuery};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

/// The adapter method that renders one cursor bound expression as text.
const CURSOR_TEXT_METHOD: &str = "render_cursor_text";

/// Selection and policy inputs for cursor warehouse inspection.
pub(crate) struct CursorInputs<'py> {
    pub(crate) selected_keys: Bound<'py, PyAny>,
    pub(crate) full_refresh_model_names: Bound<'py, PyAny>,
    pub(crate) deferred_locations: Bound<'py, PyAny>,
    pub(crate) runtime_producer_keys: Bound<'py, PyAny>,
    pub(crate) invocation_time: Bound<'py, PyAny>,
    pub(crate) start_cursor_config: Bound<'py, PyAny>,
    pub(crate) cursor_overrides: Bound<'py, PyAny>,
    pub(crate) target_relation_overrides: Option<Bound<'py, PyDict>>,
    pub(crate) origin_cursor_column_overrides: Option<Bound<'py, PyDict>>,
    pub(crate) existing_columns: Option<Bound<'py, PyDict>>,
}

struct UpstreamCursor<'py> {
    tag_min: String,
    tag_max: String,
    relation: String,
    cursor_column: String,
    input_name: String,
    cursor_grain: Option<String>,
    terminal_start: Option<Bound<'py, PyAny>>,
    terminal_end: Option<Bound<'py, PyAny>>,
}

struct CursorModel<'py> {
    model_name: String,
    target_tag: Option<String>,
    target_relation: Option<String>,
    cursor_column: String,
    origin_cursor_column: Option<String>,
    upstreams: Vec<UpstreamCursor<'py>>,
    cursor_type: Option<String>,
    effective_grain: Bound<'py, PyAny>,
    start_cursor_config: Bound<'py, PyAny>,
    has_start_override: bool,
    watermark_mode: Bound<'py, PyAny>,
    watermark_all: bool,
    watermark_strategy: bool,
}

impl CursorModel<'_> {
    fn physical_cursor_column(&self) -> &str {
        self.origin_cursor_column
            .as_deref()
            .filter(|column| !column.is_empty())
            .unwrap_or(&self.cursor_column)
    }
}

type Values<'py> = HashMap<String, Bound<'py, PyAny>>;

struct ModelMaps<'py> {
    models: Bound<'py, PyDict>,
    sources: Bound<'py, PyDict>,
    seeds: Bound<'py, PyDict>,
    functions: Bound<'py, PyDict>,
}

fn parse<'py>(
    py: Python<'py>,
    raw: &Bound<'py, PyAny>,
    cursor_type: &str,
) -> PyResult<Bound<'py, PyAny>> {
    scalars::parse(raw, &text(py, cursor_type))
}

fn render<'py>(value: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    object(value.py(), render_value(value)?)
}

fn grain<'py>(py: Python<'py>, value: &str) -> PyResult<Bound<'py, PyAny>> {
    native_member(py, RecordId::CursorGrain, value)
}

fn parse_warehouse_cursor<'py>(value: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py = value.py();
    match parse(py, value, CURSOR_TYPE_INTEGER) {
        Ok(parsed) => Ok(parsed),
        Err(error)
            if error.is_instance(
                py,
                &imported(py, CURSOR_ALGEBRA_ERROR.0, CURSOR_ALGEBRA_ERROR.1)?,
            ) =>
        {
            parse(py, value, CURSOR_TYPE_TIMESTAMP)
        }
        Err(error) => Err(error),
    }
}

fn log_debug<'py>(
    py: Python<'py>,
    message: &str,
    context: &[(&str, Bound<'py, PyAny>)],
) -> PyResult<()> {
    let logger = imported(py, "logging", "getLogger")?.call1((PLANNER_LOGGER,))?;
    let mut fields = vec![("logger", logger), ("message", text(py, message))];
    fields.extend(context.iter().cloned());
    call_with(&host::entry(py, LOG_DEBUG_EVENT)?, &fields)?;
    Ok(())
}

fn target_has_cursor_column(
    py: Python<'_>,
    (model_name, cursor_column): (&str, &str),
    existing_columns: Option<&Bound<'_, PyDict>>,
) -> PyResult<bool> {
    let columns = match existing_columns {
        Some(columns) => columns.get_item(model_name)?,
        None => None,
    };
    let Some(columns) = columns.filter(|columns| columns.is_truthy().unwrap_or(false)) else {
        return Ok(true);
    };
    let strip = |value: &Bound<'_, PyAny>| -> PyResult<String> {
        value
            .call_method1("strip", ("\"`[]",))?
            .call_method0("lower")?
            .extract()
    };
    let wanted = strip(&text(py, cursor_column))?;
    for column in columns.try_iter()? {
        if strip(&native_field(&column?, "name")?)? == wanted {
            return Ok(true);
        }
    }
    log_debug(
        py,
        "target relation lacks the cursor column; skipping its cursor bounds read",
        &[
            ("sqlbuild_model", text(py, model_name)),
            ("sqlbuild_cursor_column", text(py, cursor_column)),
        ],
    )?;
    Ok(false)
}

fn runtime_producer_names(
    project: &Bound<'_, PyAny>,
    selected: &Bound<'_, PyAny>,
) -> PyResult<HashSet<String>> {
    let mut names: HashSet<_> = HashSet::new();
    if selected.is_none() {
        return Ok(names);
    }
    let mut loaded: HashSet<String> = HashSet::new();
    for source in project_reads::sources(project)?.try_iter()? {
        let source = source?;
        if !native_field(&source_reads::source_entry(&source)?, "loader")?.is_none() {
            loaded.insert(source_reads::name(&source)?.extract()?);
        }
    }
    for key in selected.try_iter()? {
        let key = key?;
        let kind = native_field(&key, "resource_type")?;
        let name: String = native_field(&key, "name")?.extract()?;
        if kind.eq("model")? || kind.eq("seed")? || (kind.eq("source")? && loaded.contains(&name)) {
            names.insert(name);
        }
    }
    Ok(names)
}

fn upstream_relation<'py>(
    reference: &Bound<'py, PyAny>,
    adapter: &Bound<'py, PyAny>,
    maps: &ModelMaps<'py>,
    (deferred, selected): (&Bound<'py, PyAny>, Option<&HashSet<String>>),
) -> PyResult<Bound<'py, PyAny>> {
    let py = reference.py();
    let kind = native_field(reference, "ref_kind")?;
    let name = native_field(reference, "ref_name")?;
    let name_text: String = name.extract()?;
    let is_selected = selected.is_some_and(|names| names.contains(&name_text));
    let qualified = |node: Bound<'py, PyAny>| {
        native_field(&resource_reads::destination(&node)?, "qualified_name")
    };
    if (kind.eq("ref")? || kind.eq("seed")?)
        && !deferred.is_none()
        && deferred.contains(&name)?
        && !is_selected
    {
        return native_field(&deferred.get_item(&name)?, "qualified_name");
    }
    if kind.eq("ref")? {
        if let Some(model) = maps.models.get_item(&name)? {
            return qualified(model);
        }
        if let Some(seed) = maps.seeds.get_item(&name)? {
            return qualified(seed);
        }
    } else if kind.eq("seed")? {
        if let Some(seed) = maps.seeds.get_item(&name)? {
            return qualified(seed);
        }
    } else if kind.eq("source")?
        && let Some(source) = maps.sources.get_item(&name)?
    {
        return object(
            py,
            render_source_relation(
                &source_reads::source_entry(&source)?,
                Some(adapter).filter(|value| !value.is_none()),
            )?,
        );
    }
    Ok(py.None().into_bound(py))
}

fn lineage_reference<'py>(
    model: &Bound<'py, PyAny>,
    input_name: &Bound<'py, PyAny>,
    maps: &ModelMaps<'py>,
) -> PyResult<Bound<'py, PyAny>> {
    resolve_lineage_reference(
        model,
        &input_name.extract::<String>()?,
        maps.models.clone().into_any().into(),
        maps.functions.clone().into_any(),
    )
}

fn terminal_bound<'py>(
    reference: &Bound<'py, PyAny>,
    maps: &ModelMaps<'py>,
    (key, cursor_type): (&str, Option<&str>),
) -> PyResult<Option<Bound<'py, PyAny>>> {
    let py = reference.py();
    if !native_field(reference, "ref_kind")?.eq("ref")? {
        return Ok(None);
    }
    let Some(upstream) = maps.models.get_item(native_field(reference, "ref_name")?)? else {
        return Ok(None);
    };
    let value = object(py, config_cursor_bound(py, &model_values(&upstream)?, key)?)?;
    if value.is_none() {
        return Ok(None);
    }
    Ok(Some(parse(
        py,
        &value,
        cursor_type
            .filter(|value| !value.is_empty())
            .unwrap_or(CURSOR_TYPE_TIMESTAMP),
    )?))
}

fn input_grain(reference: &Bound<'_, PyAny>, maps: &ModelMaps<'_>) -> PyResult<Option<String>> {
    if !native_field(reference, "ref_kind")?.eq("ref")? {
        return Ok(None);
    }
    match maps.models.get_item(native_field(reference, "ref_name")?)? {
        Some(upstream) => config_str(&upstream, "cursor_grain"),
        None => Ok(None),
    }
}

fn start_override_set(model: &Bound<'_, PyAny>, overrides: &Bound<'_, PyAny>) -> PyResult<bool> {
    if overrides.is_none() {
        return Ok(false);
    }
    match config_str(model, "cursor_type")?.as_deref() {
        Some(CURSOR_TYPE_TIMESTAMP) => Ok(!native_field(overrides, "start_ts")?.is_none()),
        Some(CURSOR_TYPE_INTEGER) => Ok(!native_field(overrides, "start_int")?.is_none()),
        _ => Ok(false),
    }
}

fn collect_models<'py>(
    request: &SnapshotRequest<'py>,
    existing_relations: &Bound<'py, PyDict>,
    inputs: &CursorInputs<'py>,
    maps: &ModelMaps<'py>,
) -> PyResult<Vec<CursorModel<'py>>> {
    let py = request.project.py();
    let project = &request.project;
    let producers = if inputs.runtime_producer_keys.is_none() {
        inputs.selected_keys.clone()
    } else {
        inputs.runtime_producer_keys.clone()
    };
    let selected_names: Option<HashSet<String>> = if producers.is_none() {
        None
    } else {
        Some(
            producers
                .try_iter()?
                .map(|key| native_field(&key?, "name")?.extract::<String>())
                .collect::<PyResult<_>>()?,
        )
    };
    let runtime_names = runtime_producer_names(project, &producers)?;
    let mut models: Vec<_> = Vec::new();
    for model in project_reads::models(project)?.try_iter()? {
        let model = model?;
        let name_value = model_reads::name_object(&model)?;
        let model_name: String = name_value.extract()?;
        if !inputs.selected_keys.is_none()
            && !inputs.selected_keys.contains(model_key(py, &name_value)?)?
        {
            continue;
        }
        let cursor_column = config_str(&model, "cursor")?;
        let materialized = config_str(&model, "materialized")?;
        let Some(cursor_column) =
            cursor_column.filter(|_| materialized.as_deref() == Some("incremental"))
        else {
            continue;
        };
        let watermark_inputs =
            native_field(&resolve_cursor_input_roles(&model)?, "watermark_inputs")?;
        let watermark_inputs: Vec<(Bound<'py, PyAny>, Bound<'py, PyAny>)> =
            mapping_items(&watermark_inputs)?
                .try_iter()?
                .map(|item| item?.extract())
                .collect::<PyResult<_>>()?;
        if inputs.full_refresh_model_names.contains(&name_value)? {
            for (input_name, upstream_column) in &watermark_inputs {
                let reference = lineage_reference(&model, input_name, maps)?;
                validate_watermark_column(
                    &model,
                    &reference,
                    &upstream_column.extract::<String>()?,
                    maps_tuple(maps),
                )?;
            }
            continue;
        }
        let mut target_tag = None;
        let mut target_relation = None;
        let redirected = match &inputs.target_relation_overrides {
            Some(overrides) => overrides.get_item(&name_value)?,
            None => None,
        };
        let target_info = existing_relations.get_item(&name_value)?;
        let qualified = native_field(&model_reads::destination(&model)?, "qualified_name")?;
        if let Some(redirected) = redirected.filter(|value| !value.is_none()) {
            target_tag = Some(format!("{model_name}__target__max"));
            target_relation = Some(redirected.extract::<String>()?);
        } else if !qualified.is_none()
            && let Some(info) = target_info
            && native_field(&info, "name")?.eq(&name_value)?
        {
            if target_has_cursor_column(
                py,
                (&model_name, &cursor_column),
                inputs.existing_columns.as_ref(),
            )? {
                target_tag = Some(format!("{model_name}__target__max"));
            }
            target_relation = Some(qualified.extract::<String>()?);
        }
        let cursor_type = config_str(&model, "cursor_type")?;
        let cursor_grain = config_str(&model, "cursor_grain")?;
        let mut upstreams: Vec<_> = Vec::new();
        for (input_name, upstream_column) in &watermark_inputs {
            let upstream_column: String = upstream_column.extract()?;
            let reference = lineage_reference(&model, input_name, maps)?;
            validate_watermark_column(&model, &reference, &upstream_column, maps_tuple(maps))?;
            let ref_name: String = native_field(&reference, "ref_name")?.extract()?;
            if runtime_names.contains(&ref_name) {
                continue;
            }
            let relation = upstream_relation(
                &reference,
                &request.adapter,
                maps,
                (&inputs.deferred_locations, selected_names.as_ref()),
            )?;
            if relation.is_none() {
                continue;
            }
            if !existing_relations.contains(&ref_name)?
                && !native_field(&reference, "ref_kind")?.eq("source")?
            {
                continue;
            }
            upstreams.push(UpstreamCursor {
                tag_min: format!("{model_name}__{ref_name}__min"),
                tag_max: format!("{model_name}__{ref_name}__max"),
                relation: relation.extract()?,
                cursor_column: upstream_column,
                input_name: ref_name,
                cursor_grain: input_grain(&reference, maps)?,
                terminal_start: terminal_bound(
                    &reference,
                    maps,
                    ("cursor_start", cursor_type.as_deref()),
                )?,
                terminal_end: terminal_bound(
                    &reference,
                    maps,
                    ("cursor_end", cursor_type.as_deref()),
                )?,
            });
        }
        let strategy = config_str(&model, "microbatch_strategy")?;
        let mut input_grains: Vec<_> = Vec::new();
        for upstream in &upstreams {
            input_grains.push(match &upstream.cursor_grain {
                Some(value) => grain(py, value)?,
                None => py.None().into_bound(py),
            });
        }
        let effective_grain = resolve_effective_timestamp_grain(
            &optional_text(py, cursor_type.as_deref()),
            &optional_text(py, cursor_grain.as_deref()),
            &tuple(py, input_grains)?,
            optional_text(py, strategy.as_deref())
                .extract::<Option<String>>()?
                .as_deref(),
        )?;
        let mode = config_str(&model, "cursor_watermark_mode")?
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "all".to_owned());
        let watermark_mode = native_member(py, RecordId::CursorWatermarkMode, &mode)?;
        models.push(CursorModel {
            target_tag,
            target_relation,
            origin_cursor_column: match &inputs.origin_cursor_column_overrides {
                Some(overrides) => overrides
                    .get_item(&name_value)?
                    .map(|value| value.extract())
                    .transpose()?,
                None => None,
            },
            cursor_column,
            upstreams,
            effective_grain,
            start_cursor_config: resolve_start_cursor_config(&model, &inputs.start_cursor_config)?,
            has_start_override: start_override_set(&model, &inputs.cursor_overrides)?,
            watermark_all: watermark_mode.eq("all")?,
            watermark_mode,
            watermark_strategy: strategy.as_deref() == Some("watermark"),
            cursor_type,
            model_name,
        });
    }
    Ok(models)
}

fn maps_tuple<'a, 'py>(
    maps: &'a ModelMaps<'py>,
) -> (
    &'a Bound<'py, PyDict>,
    &'a Bound<'py, PyDict>,
    &'a Bound<'py, PyDict>,
) {
    (&maps.models, &maps.sources, &maps.seeds)
}

fn physical_queries(models: &[CursorModel<'_>]) -> Vec<PhysicalCursorQuery> {
    let mut requests: Vec<_> = Vec::new();
    for info in models {
        if let (Some(tag), Some(relation)) = (&info.target_tag, &info.target_relation) {
            requests.push(CursorTagRequest {
                relation: relation.clone(),
                cursor_column: info.physical_cursor_column().to_owned(),
                cursor_type: info.cursor_type.clone(),
                min_tag: None,
                max_tag: tag.clone(),
            });
        }
        for upstream in &info.upstreams {
            requests.push(CursorTagRequest {
                relation: upstream.relation.clone(),
                cursor_column: upstream.cursor_column.clone(),
                cursor_type: info.cursor_type.clone(),
                min_tag: Some(upstream.tag_min.clone()),
                max_tag: upstream.tag_max.clone(),
            });
        }
    }
    cursor_query_groups(&requests)
}

/// Run one physical cursor statement; a failure leaves only that relation unavailable.
fn read_cursor_query<'py>(
    py: Python<'py>,
    sql: &str,
    (connection, execute): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
) -> PyResult<Bound<'py, PyAny>> {
    let started = Instant::now();
    let outcome = call_with(
        execute,
        &[("connection", connection.clone()), ("sql", text(py, sql))],
    )
    .and_then(|result| result.call_method0("fetchall"));
    let elapsed = started.elapsed().as_secs_f64();
    let (row, error, count) = match outcome {
        Ok(rows) => {
            let count = rows.len()?;
            let row = if count > 0 {
                rows.get_item(0)?
            } else {
                py.None().into_bound(py)
            };
            (
                row,
                py.None().into_bound(py),
                count.into_pyobject(py)?.into_any(),
            )
        }
        Err(error) if error.is_instance_of::<PyException>(py) => {
            let value = error.value(py).clone().into_any();
            (py.None().into_bound(py), value, py.None().into_bound(py))
        }
        Err(error) => return Err(error),
    };
    let error_text = if error.is_none() {
        py.None().into_bound(py)
    } else {
        error.str()?.into_any()
    };
    let record = call_with(
        &host::entry(py, QUERY_RECORD)?,
        &[
            ("sql", text(py, sql)),
            ("elapsed_seconds", PyFloat::new(py, elapsed).into_any()),
            ("row_count", count),
            ("error", error_text),
        ],
    )?;
    call_with(&host::entry(py, RECORD_QUERY)?, &[("record", record)])?;
    Ok(PyTuple::new(py, [row, PyFloat::new(py, elapsed).into_any(), error])?.into_any())
}

fn report_cursor_query(
    py: Python<'_>,
    query: &PhysicalCursorQuery,
    (outcome, identity): (&Bound<'_, PyAny>, &str),
    on_progress: &Bound<'_, PyAny>,
) -> PyResult<()> {
    let elapsed: f64 = outcome.get_item(1)?.extract()?;
    let error = outcome.get_item(2)?;
    if error.is_none() {
        if !on_progress.is_none() {
            on_progress.call1((format!(
                "Inspected cursor bounds {identity} ({elapsed:.2}s)"
            ),))?;
        }
        return Ok(());
    }
    let error_text: String = error.str()?.extract()?;
    if !on_progress.is_none() {
        on_progress.call1((format!(
            "Failed cursor bounds {identity} ({elapsed:.2}s): {error_text}"
        ),))?;
    }
    log_debug(
        py,
        "cursor bounds physical query failed; treating relation as unavailable",
        &[
            ("sqlbuild_relation", text(py, &query.relation)),
            ("sqlbuild_cursor_column", text(py, &query.cursor_column)),
            ("sqlbuild_bounds", text(py, &cursor_bounds(query))),
            (
                "sqlbuild_elapsed_seconds",
                text(py, &format!("{elapsed:.2}")),
            ),
            ("sqlbuild_error", text(py, &error_text)),
        ],
    )
}

fn bound_task<'py>(
    py: Python<'py>,
    sql: String,
    (connection, execute): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
) -> PyResult<Bound<'py, PyAny>> {
    let connection: Py<PyAny> = connection.clone().unbind();
    let execute: Py<PyAny> = execute.clone().unbind();
    Ok(PyCFunction::new_closure(
        py,
        None,
        None,
        move |args: &Bound<'_, PyTuple>,
              _kwargs: Option<&Bound<'_, PyDict>>|
              -> PyResult<Py<PyAny>> {
            let py = args.py();
            Ok(read_cursor_query(py, &sql, (connection.bind(py), execute.bind(py)))?.unbind())
        },
    )?
    .into_any())
}

fn completion_reporter<'py>(
    py: Python<'py>,
    queries: Vec<PhysicalCursorQuery>,
    on_progress: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let completed = Arc::new(AtomicUsize::new(0));
    let on_progress: Py<PyAny> = on_progress.clone().unbind();
    Ok(PyCFunction::new_closure(
        py,
        None,
        None,
        move |args: &Bound<'_, PyTuple>, kwargs: Option<&Bound<'_, PyDict>>| -> PyResult<()> {
            let py = args.py();
            let kwargs = kwargs.cloned().unwrap_or_else(|| PyDict::new(py));
            let index: usize = kwargs
                .get_item("index")?
                .map_or(Ok(0), |value| value.extract())?;
            let result = kwargs
                .get_item("result")?
                .unwrap_or_else(|| py.None().into_bound(py));
            let done = completed.fetch_add(1, Ordering::SeqCst) + 1;
            let query = &queries[index];
            let identity = cursor_query_identity(query, (done, queries.len()));
            report_cursor_query(py, query, (&result, &identity), on_progress.bind(py))
        },
    )?
    .into_any())
}

fn fan_out<'py>(query: &PhysicalCursorQuery, row: &Bound<'py, PyAny>) -> PyResult<Values<'py>> {
    let mut values = Values::new();
    if row.is_none() {
        return Ok(values);
    }
    let mut index = 0;
    if !query.min_tags.is_empty() {
        let value = row.get_item(index)?;
        if !value.is_none() {
            for tag in &query.min_tags {
                values.insert(tag.clone(), parse_warehouse_cursor(&value)?);
            }
        }
        index += 1;
    }
    if !query.max_tags.is_empty() {
        let value = row.get_item(index)?;
        if !value.is_none() {
            for tag in &query.max_tags {
                values.insert(tag.clone(), parse_warehouse_cursor(&value)?);
            }
        }
    }
    Ok(values)
}

/// The query's MIN/MAX statement, each bound read as text the way the adapter renders it.
fn adapter_cursor_sql(
    request: &SnapshotRequest<'_>,
    query: &PhysicalCursorQuery,
) -> PyResult<String> {
    let py = request.adapter.py();
    let render = native_field(&request.adapter, CURSOR_TEXT_METHOD)?;
    let failure: RefCell<Option<PyErr>> = RefCell::new(None);
    let sql: String = cursor_query_sql(query, &|expression: &str| {
        call_with(
            &render,
            &[
                ("expression", text(py, expression)),
                ("cursor_type", text(py, &query.cursor_type)),
            ],
        )
        .and_then(|rendered| rendered.extract::<String>())
        .unwrap_or_else(|error| {
            failure.replace(Some(error));
            String::new()
        })
    });
    match failure.into_inner() {
        Some(error) => Err(error),
        None => Ok(sql),
    }
}

fn execute_queries<'py>(
    request: &SnapshotRequest<'py>,
    queries: &[PhysicalCursorQuery],
    concurrency: i64,
) -> PyResult<(Values<'py>, HashSet<String>)> {
    let py = request.project.py();
    let total = queries.len();
    let on_progress = &request.on_progress;
    let handles = (&request.connection, &request.execute);
    let outcomes: Vec<Bound<'py, PyAny>> = if concurrency <= 1 || total <= 1 {
        let mut outcomes: Vec<_> = Vec::new();
        for (position, query) in queries.iter().enumerate() {
            let identity = cursor_query_identity(query, (position + 1, total));
            if !on_progress.is_none() {
                on_progress.call1((format!("Inspecting cursor bounds {identity}..."),))?;
            }
            let outcome = read_cursor_query(py, &adapter_cursor_sql(request, query)?, handles)?;
            report_cursor_query(py, query, (&outcome, &identity), on_progress)?;
            outcomes.push(outcome);
        }
        outcomes
    } else {
        if !on_progress.is_none() {
            on_progress.call1((format!(
                "Inspecting cursor bounds for {total} relations ({} concurrent)...",
                (concurrency as usize).min(total)
            ),))?;
        }
        let mut tasks: Vec<_> = Vec::new();
        for query in queries {
            tasks.push(bound_task(
                py,
                adapter_cursor_sql(request, query)?,
                handles,
            )?);
        }
        call_with(
            &host::entry(py, RUN_BOUNDED)?,
            &[
                ("tasks", tuple(py, tasks)?),
                ("concurrency", concurrency.into_pyobject(py)?.into_any()),
                (
                    "on_complete",
                    completion_reporter(py, queries.to_vec(), on_progress)?,
                ),
            ],
        )?
        .try_iter()?
        .collect::<PyResult<_>>()?
    };
    let mut values = Values::new();
    let mut failed: HashSet<_> = HashSet::new();
    for (query, outcome) in queries.iter().zip(outcomes) {
        if !outcome.get_item(2)?.is_none() {
            failed.extend(query.min_tags.iter().cloned());
            failed.extend(query.max_tags.iter().cloned());
        }
        values.extend(fan_out(query, &outcome.get_item(0)?)?);
    }
    Ok((values, failed))
}

fn eligible_task<'py>(
    py: Python<'py>,
    (model_name, target, cursor_column): (String, String, String),
    sql: String,
    (connection, execute): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
) -> PyResult<Bound<'py, PyAny>> {
    let connection: Py<PyAny> = connection.clone().unbind();
    let execute: Py<PyAny> = execute.clone().unbind();
    Ok(PyCFunction::new_closure(
        py,
        None,
        None,
        move |args: &Bound<'_, PyTuple>, _kwargs: Option<&Bound<'_, PyDict>>| -> PyResult<Py<PyAny>> {
            let py = args.py();
            let run_connection = connection.clone_ref(py);
            let run_execute = execute.clone_ref(py);
            let run_sql = sql.clone();
            let run = PyCFunction::new_closure(
                py,
                None,
                None,
                move |args: &Bound<'_, PyTuple>, _kwargs: Option<&Bound<'_, PyDict>>| -> PyResult<Py<PyAny>> {
                    let py = args.py();
                    Ok(run_execute
                        .bind(py)
                        .call((), Some(&keywords(py, &[
                            ("connection", run_connection.bind(py).clone()),
                            ("sql", text(py, &run_sql)),
                        ])?))?
                        .call_method0("fetchall")?
                        .unbind())
                },
            )?;
            let rows = call_with(
                &host::entry(py, RUN_RECORDED_QUERY)?,
                &[("sql", text(py, &sql)), ("run", run.into_any())],
            );
            match rows {
                Ok(rows) => Ok(rows.unbind()),
                Err(error) if error.is_instance_of::<PyException>(py) => {
                    let wrapped: PyErr = planner_error(
                        py,
                        &format!(
                            "model '{model_name}': failed to query highest eligible target cursor for {target}.{cursor_column}: {}",
                            error.value(py).str()?
                        ),
                        (None, None),
                    );
                    wrapped.set_cause(py, Some(error));
                    Err(wrapped)
                }
                Err(error) => Err(error),
            }
        },
    )?
    .into_any())
}

fn eligible_target_maxes<'py>(
    request: &SnapshotRequest<'py>,
    models: &[CursorModel<'py>],
    mut values: Values<'py>,
    invocation_time: &Bound<'py, PyAny>,
) -> PyResult<Values<'py>> {
    let py = request.project.py();
    if invocation_time.is_none() {
        return Ok(values);
    }
    let mut requests: Vec<(&CursorModel<'py>, Bound<'py, PyAny>)> = Vec::new();
    for info in models {
        let config = &info.start_cursor_config;
        let target_max = info.target_tag.as_ref().and_then(|tag| values.get(tag));
        let Some(target_max) = target_max else {
            continue;
        };
        if config.is_none()
            || native_field(config, "max_ahead")?.is_none()
            || info.cursor_type.as_deref() != Some(CURSOR_TYPE_TIMESTAMP)
            || info.target_relation.is_none()
            || info.has_start_override
        {
            continue;
        }
        let allowed = text(
            py,
            &maximum_allowed_start(
                py,
                Some(keywords(
                    py,
                    &[
                        ("discovered_value", render(target_max)?),
                        ("cursor_grain", info.effective_grain.clone()),
                        ("invocation_time", invocation_time.clone()),
                        ("max_ahead", native_field(config, "max_ahead")?),
                    ],
                )?),
            )?,
        );
        let horizon = parse(py, &allowed, CURSOR_TYPE_TIMESTAMP)?;
        let order: i64 = object(py, compare_values(target_max, &horizon)?)?.extract()?;
        if order <= 0 {
            continue;
        }
        let sql = call_with(
            &native_field(&request.adapter, "render_max_cursor_at_or_before")?,
            &[
                (
                    "relation",
                    optional_text(py, info.target_relation.as_deref()),
                ),
                ("cursor_column", text(py, info.physical_cursor_column())),
                ("maximum_allowed", render(&horizon)?),
                (
                    "cursor_type",
                    optional_text(py, info.cursor_type.as_deref()),
                ),
                (
                    "is_date",
                    horizon
                        .is_instance(&record_class(py, DATE_VALUE)?)?
                        .into_pyobject(py)?
                        .to_owned()
                        .into_any(),
                ),
            ],
        )?;
        requests.push((info, sql));
    }
    let mut tasks: Vec<_> = Vec::new();
    for (info, sql) in &requests {
        tasks.push(eligible_task(
            py,
            (
                info.model_name.clone(),
                info.target_relation.clone().unwrap_or_default(),
                info.cursor_column.clone(),
            ),
            sql.extract()?,
            (&request.connection, &request.execute),
        )?);
    }
    let rows_by_request = call_with(
        &host::entry(py, RUN_BOUNDED)?,
        &[
            ("tasks", tuple(py, tasks)?),
            (
                "concurrency",
                inspection_concurrency(&request.adapter, &request.connection)?
                    .into_pyobject(py)?
                    .into_any(),
            ),
        ],
    )?;
    for ((info, _), rows) in requests.iter().zip(rows_by_request.try_iter()?) {
        let rows = rows?;
        if !rows.is_truthy()? {
            continue;
        }
        let first = rows.get_item(0)?.get_item(0)?;
        if first.is_none() {
            continue;
        }
        let cursor_type = info
            .cursor_type
            .as_deref()
            .filter(|value| !value.is_empty())
            .unwrap_or(CURSOR_TYPE_TIMESTAMP);
        let value = parse(py, &first, cursor_type)?;
        let normalized = normalize_cursor_snapshot_grain(
            &planner_model(
                py,
                RecordId::ModelCursorSnapshot,
                &[
                    ("target_max", py.None().into_bound(py)),
                    ("upstream_mins", PyTuple::empty(py).into_any()),
                    ("upstream_maxes", PyTuple::empty(py).into_any()),
                    ("target_eligible_max", value.clone()),
                ],
            )?,
            &optional_text(py, info.cursor_type.as_deref()),
            &info.effective_grain.clone(),
        )?
        .getattr("target_eligible_max")?;
        values.insert(
            format!("{}__target__eligible_max", info.model_name),
            if normalized.is_truthy()? {
                normalized
            } else {
                value
            },
        );
    }
    Ok(values)
}

fn cursor_grain<'py>(
    py: Python<'py>,
    info: &CursorModel<'py>,
    upstream: &UpstreamCursor<'py>,
) -> PyResult<Bound<'py, PyAny>> {
    if info.cursor_type.as_deref() == Some(CURSOR_TYPE_INTEGER) {
        return Ok(py.None().into_bound(py));
    }
    let value = match upstream
        .cursor_grain
        .as_deref()
        .filter(|value| !value.is_empty())
    {
        Some(value) => value.to_owned(),
        None if info.effective_grain.is_truthy()? => info.effective_grain.str()?.extract()?,
        None => "second".to_owned(),
    };
    grain(py, &value)
}

fn optional<'py>(py: Python<'py>, value: Option<&Bound<'py, PyAny>>) -> Bound<'py, PyAny> {
    value.cloned().unwrap_or_else(|| py.None().into_bound(py))
}

fn assemble<'py>(
    py: Python<'py>,
    models: &[CursorModel<'py>],
    (values, failed): (&Values<'py>, &HashSet<String>),
) -> PyResult<Bound<'py, PyDict>> {
    let snapshots = PyDict::new(py);
    for info in models {
        let target_max = info.target_tag.as_ref().and_then(|tag| values.get(tag));
        let mut mins: Vec<_> = Vec::new();
        let mut maxes: Vec<_> = Vec::new();
        let mut evidence: Vec<_> = Vec::new();
        let mut unavailable: Vec<_> = Vec::new();
        let mut starts: Vec<_> = Vec::new();
        let mut ends: Vec<_> = Vec::new();
        let mut end_inputs: Vec<_> = Vec::new();
        let mut availability: Vec<_> = Vec::new();
        let mut empty: Vec<_> = Vec::new();
        let mut unreadable: Vec<_> = Vec::new();
        for upstream in &info.upstreams {
            let min_value = values.get(&upstream.tag_min);
            let max_value = values.get(&upstream.tag_max);
            if max_value.is_none() && upstream.terminal_end.is_none() {
                let input = if upstream.input_name.is_empty() {
                    &upstream.relation
                } else {
                    &upstream.input_name
                };
                let label = text(py, &format!("{input}.{}", upstream.cursor_column));
                if failed.contains(&upstream.tag_max) {
                    unreadable.push(label);
                } else {
                    empty.push(label);
                }
            }
            let effective_max = match &upstream.terminal_end {
                Some(end) => Some(to_inclusive(end, &cursor_grain(py, info, upstream)?)?),
                None => max_value.cloned(),
            };
            if let Some(min_value) = min_value {
                mins.push(min_value.clone());
            } else if let Some(start) = &upstream.terminal_start {
                mins.push(start.clone());
            } else if target_max.is_none() && info.watermark_all {
                unavailable.push(text(py, &upstream.tag_min));
            }
            if let Some(effective_max) = effective_max {
                if let Some(max_value) = max_value {
                    maxes.push(max_value.clone());
                }
                let minimum = match min_value {
                    Some(min_value) if min_value.is_truthy()? => min_value.clone(),
                    _ => optional(py, upstream.terminal_start.as_ref()),
                };
                evidence.push(planner_model(
                    py,
                    RecordId::CursorInputEvidence,
                    &[
                        ("relation", text(py, &upstream.relation)),
                        ("cursor_column", text(py, &upstream.cursor_column)),
                        ("minimum", minimum),
                        ("maximum", effective_max),
                    ],
                )?);
            } else if upstream.terminal_end.is_none() && info.watermark_all {
                unavailable.push(text(py, &upstream.tag_max));
            }
            if let Some(start) = &upstream.terminal_start {
                starts.push(start.clone());
            }
            if let Some(end) = &upstream.terminal_end {
                ends.push(end.clone());
            }
            end_inputs.push(
                PyTuple::new(
                    py,
                    [
                        optional(py, max_value),
                        optional(py, upstream.terminal_end.as_ref()),
                    ],
                )?
                .into_any(),
            );
            if info.watermark_strategy {
                let mut available = upstream.terminal_end.clone();
                if available.is_none()
                    && let Some(max_value) = max_value
                {
                    available = Some(native_field(
                        &partition(max_value, &cursor_grain(py, info, upstream)?)?,
                        "end",
                    )?);
                }
                if let Some(available) = available.filter(|value| !value.is_none()) {
                    availability.push(available);
                }
            }
        }
        let end_inputs = if ends.is_empty() {
            Vec::new()
        } else {
            end_inputs
        };
        snapshots.set_item(
            &info.model_name,
            planner_model(
                py,
                RecordId::ModelCursorSnapshot,
                &[
                    ("target_max", optional(py, target_max)),
                    ("upstream_mins", tuple(py, mins)?),
                    ("upstream_maxes", tuple(py, maxes)?),
                    ("physical_target_max", optional(py, target_max)),
                    (
                        "target_eligible_max",
                        optional(
                            py,
                            values.get(&format!("{}__target__eligible_max", info.model_name)),
                        ),
                    ),
                    (
                        "target_relation",
                        optional_text(py, info.target_relation.as_deref()),
                    ),
                    ("destination_cursor_column", text(py, &info.cursor_column)),
                    ("input_evidence", tuple(py, evidence)?),
                    (
                        "expected_watermark_count",
                        info.upstreams.len().into_pyobject(py)?.into_any(),
                    ),
                    ("unavailable_watermark_tags", tuple(py, unavailable)?),
                    ("cursor_watermark_mode", info.watermark_mode.clone()),
                    ("upstream_terminal_starts", tuple(py, starts)?),
                    ("upstream_terminal_ends", tuple(py, ends)?),
                    ("upstream_end_inputs", tuple(py, end_inputs)?),
                    ("upstream_availability_ends", tuple(py, availability)?),
                    ("empty_input_names", tuple(py, empty)?),
                    ("unreadable_input_names", tuple(py, unreadable)?),
                ],
            )?,
        )?;
    }
    Ok(snapshots)
}

/// Cursor MIN/MAX values of the selected incremental models, by model name.
pub(crate) fn cursor_snapshots<'py>(
    request: &SnapshotRequest<'py>,
    existing_relations: &Bound<'py, PyDict>,
    inputs: &CursorInputs<'py>,
) -> PyResult<Bound<'py, PyDict>> {
    let py = request.project.py();
    let maps = ModelMaps {
        models: by_name(&request.project, "models")?,
        sources: by_name(&request.project, "sources")?,
        seeds: by_name(&request.project, "seeds")?,
        functions: by_name(&request.project, "functions")?,
    };
    let models = collect_models(request, existing_relations, inputs, &maps)?;
    if models.is_empty() {
        return Ok(PyDict::new(py));
    }
    let queries = physical_queries(&models);
    let started = Instant::now();
    let concurrency = inspection_concurrency(&request.adapter, &request.connection)?;
    let (values, failed) = execute_queries(request, &queries, concurrency)?;
    let values = eligible_target_maxes(request, &models, values, &inputs.invocation_time)?;
    if !request.on_progress.is_none() {
        let logical: usize = queries
            .iter()
            .map(|query| query.min_tags.len() + query.max_tags.len())
            .sum();
        request.on_progress.call1((format!(
            "Gathered cursor bounds ({}/{logical} logical values; {} physical relation reads). ({:.2}s)",
            values.len(),
            queries.len(),
            started.elapsed().as_secs_f64()
        ),))?;
    }
    assemble(py, &models, (&values, &failed))
}
