//! Per-model plan entries from resolved change results and warehouse snapshot facts.

use crate::planner_changes::main::replaces_incremental_relation::replaces_incremental_relation;
use crate::planner_entries::_helpers::entries::values::{
    MICROBATCH_CONCURRENCY_MESSAGE, error_of, native, none, planner_type, replaced, string,
};
use crate::planner_entries::models::{
    ModelPlanContext, PlanEntriesRequest, PlanEntryBuildInputs, PlannerRelationsContext,
};
use crate::planner_phases::main::inputs::class_field::class_field;
use crate::planner_phases::main::rebuilds::check_full_rebuild_allowed::check_full_rebuild_allowed;
use crate::planner_phases::main::rebuilds::describe_full_rebuild_cause::describe_full_rebuild_cause;
use crate::planner_phases::main::rebuilds::resolve_model_full_refresh::resolve_model_full_refresh;
use crate::planner_phases::models::ModelEntryResults;
use crate::planner_resolution::main::cursor::config_str::config_str;
use crate::planner_resolution::main::cursor::cursor_safety_configs::cursor_safety_configs;
use crate::planner_resolution::main::cursor::empty_cursor_inputs::empty_cursor_inputs;
use crate::planner_resolution::main::cursor::microbatch_range::microbatch_range;
use crate::planner_resolution::main::cursor::model_cursor_overrides::model_cursor_overrides;
use crate::planner_resolution::main::cursor::plan_cursor_bounds::plan_cursor_bounds;
use crate::planner_resolution::main::cursor::with_cursor_safety_warnings::with_cursor_safety_warnings;
use crate::planner_resolution::main::references::model_hook_entries::model_hook_entries;
use crate::planner_resolution::main::references::resolve_model_sql::resolve_model_sql;
use crate::planner_resolution::models::CursorWindowInputs;
use crate::planner_resolution::models::ModelSqlRequest as SqlRequest;
use crate::planner_resolution::models::Resolution;
use crate::planner_strategy::main::cursor_inputs::build_cursor_input_relations::build_cursor_input_relations;
use crate::planner_strategy::main::cursor_inputs::reject_unexposed_cursor_input_columns::reject_unexposed_cursor_input_columns;
use crate::planner_strategy::main::cursor_inputs::runtime_cursor_producer_names::runtime_cursor_producer_names;
use crate::planner_strategy::main::lineage::reject_capped_watermark_inputs::reject_capped_watermark_inputs;
use crate::planner_strategy::main::microbatch_limits::apply_configured_microbatch_limits::apply_configured_microbatch_limits;
use crate::planner_strategy::main::model_entries::assemble_model_plan_entry::assemble_model_plan_entry;
use crate::planner_strategy::main::strategy::materialization_of::materialization_of;
use crate::planner_strategy::main::strategy::resolve_model_plan_action::resolve_model_plan_action;
use crate::planner_strategy::main::values::text::text;
use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyResult};
use pyo3::pyfunction;
use pyo3::types::{PyDict, PyDictMethods};
use sqlbuild_python_base::boundary::main::keywords::keywords;
use sqlbuild_python_base::records::main::construct::enum_member::enum_member;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::ErrorId;
use sqlbuild_python_base::records::types::RecordId;
use sqlbuild_python_front::discovery::main::failures::microbatch_guidance::microbatch_guidance;
use sqlbuild_python_project::compiled_views::main::compiled_models::name_object::name_object;
use sqlbuild_python_project::compiled_views::main::project::functions::functions;
use sqlbuild_python_project::planner_phases::main::object::object;
use sqlbuild_python_project::planner_strategy::models::{CursorInputLookups, ModelEntryDecisions};
use sqlbuild_python_project::semantic_shapes::main::source_binding_diagnostics::source_binding_diagnostics;

const MICROBATCH_COMPLEXITY_CODE: &str = "MICROBATCH_CONCURRENCY_COMPLEXITY";

/// The `ModelPlanContext` fields every model's planning reads, read once per plan.
pub(crate) struct ContextReads<'py> {
    context: Bound<'py, PyAny>,
    lookups: CursorInputLookups<'py>,
    resolution: Resolution<'py>,
    invocation_time: Bound<'py, PyAny>,
}

impl<'py> ContextReads<'py> {
    pub(crate) fn of(
        context: Bound<'py, PyAny>,
        adapter: Bound<'py, PyAny>,
        external_resolver: Bound<'py, PyAny>,
    ) -> PyResult<Self> {
        let py = context.py();
        let fields = context.cast::<ModelPlanContext>()?.get();
        let field = |value: &Py<PyAny>| value.bind(py).clone();
        Ok(Self {
            lookups: CursorInputLookups {
                models_by_name: fields.models_by_name.clone_ref(py),
                functions_by_name: field(&fields.functions_by_name),
                source_map: field(&fields.source_map),
                source_warehouse_columns: field(&fields.source_warehouse_columns),
                model_locations: field(&fields.model_locations),
                seed_locations: field(&fields.seed_locations),
                runtime_cursor_producer_names: field(&fields.runtime_cursor_producer_names),
            },
            resolution: Resolution::of_context(adapter, &context, external_resolver)?,
            invocation_time: field(&fields.invocation_time),
            context,
        })
    }
}

/// One model's planning request after change detection.
pub(crate) struct ModelPlanRequest<'a, 'py> {
    pub(crate) model: Bound<'py, PyAny>,
    pub(crate) snapshot: Bound<'py, PyAny>,
    pub(crate) context: &'a ContextReads<'py>,
    pub(crate) sql_analysis_enabled: bool,
    pub(crate) full_refresh: bool,
    pub(crate) overrides: (Bound<'py, PyAny>, Bound<'py, PyAny>),
    pub(crate) change_result: Bound<'py, PyAny>,
    pub(crate) protect_full_refresh: bool,
}

/// Build a model plan entry and its warnings from a resolved change result.
pub(crate) fn plan_model_from_change<'py>(
    request: &ModelPlanRequest<'_, 'py>,
) -> PyResult<(Bound<'py, PyAny>, Bound<'py, PyAny>)> {
    let model = &request.model;
    let reads = request.context;
    let context = &reads.context;
    let adapter = &reads.resolution.adapter;
    let py = model.py();
    let backfill = if request.full_refresh {
        native(
            py,
            RecordId::BackfillResult,
            &[(
                "action",
                planner_type(py, RecordId::BackfillAction, "FULL")?,
            )],
        )?
    } else {
        native_field(&request.change_result, "backfill")?
    };
    let cursor_column = config_str(model, "cursor")?;
    reject_unexposed_cursor_input_columns(model, &cursor_column, &reads.lookups)?;
    let resolved = resolve_model_plan_action(model, &request.change_result, request.full_refresh)?;
    let (action, reason) = (resolved.get_item(0)?, resolved.get_item(1)?);
    let materialization_type = materialization_of(model)?;
    let replaces_relation =
        replaces_incremental_relation(&text(&materialization_type)?, &text(&action)?);
    let full_rebuild_cause = if replaces_relation {
        let cause: String =
            describe_full_rebuild_cause(&request.change_result, request.full_refresh)?;
        if request.protect_full_refresh && !request.full_refresh {
            check_full_rebuild_allowed(model, &request.change_result, &cause)?;
        }
        Some(cause)
    } else {
        None
    };
    let resolved_sql = resolve_model_sql(
        &reads.resolution,
        context,
        &SqlRequest {
            model: model.clone(),
            snapshot: request.snapshot.clone(),
            backfill: backfill.clone(),
            full_refresh: request.full_refresh,
            cursor_overrides: request.overrides.clone(),
            replaces_relation,
            full_rebuild_cause,
        },
    )?;
    let hooks = model_hook_entries(model, &reads.resolution)?;
    let (future_config, start_config) = cursor_safety_configs(model, context)?;
    reject_capped_watermark_inputs(
        model,
        reads.lookups.models_by_name.clone_ref(py),
        Some(reads.lookups.functions_by_name.clone()),
    )?;
    let cursor_input_relations =
        build_cursor_input_relations((model, adapter), &cursor_column, &reads.lookups)?.into_any();
    let mut any_runtime_owned = false;
    for relation in cursor_input_relations.try_iter()? {
        if native_field(&relation?, "is_runtime_owned")?.is_truthy()? {
            any_runtime_owned = true;
            break;
        }
    }
    let complete_override = !request.overrides.0.is_none() && !request.overrides.1.is_none();
    let runtime_owned = any_runtime_owned && !complete_override;
    let window = CursorWindowInputs {
        snapshot: request.snapshot.clone(),
        backfill: backfill.clone(),
        overrides: request.overrides.clone(),
        runtime_owned,
        policies: (future_config.clone(), start_config.clone()),
        invocation_time: reads.invocation_time.clone(),
        full_refresh: request.full_refresh,
        replaces_relation,
        cursor_input_relations: cursor_input_relations.clone(),
    };
    let cursor_bounds = plan_cursor_bounds(model, &window)?;
    let (empty_inputs, waiting_on_empty_inputs) = empty_cursor_inputs(model, &window)?;
    let range = microbatch_range(model, &window)?;
    let (entry, entry_warnings) = assemble_model_plan_entry(&ModelEntryDecisions {
        model: model.clone(),
        adapter: adapter.clone(),
        snapshot: request.snapshot.clone(),
        change_result: request.change_result.clone(),
        action,
        reason,
        materialization_type,
        resolved_sql: string(py, &resolved_sql),
        sql_analysis_enabled: request.sql_analysis_enabled,
        hooks: (hooks.get_item(0)?, hooks.get_item(1)?),
        cursor_column,
        cursor_input_relations,
        runtime_owned_cursor_bounds: runtime_owned,
        cursor_bounds: cursor_bounds.clone(),
        microbatch_range: range.clone(),
        empty_cursor_inputs: empty_inputs,
        waiting_on_empty_inputs,
        backfill,
        cursor_overrides: request.overrides.clone(),
        cursor_configs: (future_config, start_config),
        invocation_time: reads.invocation_time.clone(),
    })?;
    let bounds = if range.is_truthy()? {
        range
    } else {
        cursor_bounds
    };
    let warnings =
        with_cursor_safety_warnings(&entry_warnings.into_any(), &name_object(model)?, &bounds)?;
    Ok((entry, warnings))
}

/// The binding-diagnostic failure that stops planning, if any diagnostic is an error.
fn check_binding_diagnostics<'py>(
    project: &Bound<'py, PyAny>,
    adapter: &Bound<'py, PyAny>,
    relations: &Bound<'py, PyAny>,
    selected_keys: &Bound<'py, PyAny>,
) -> PyResult<()> {
    let py = project.py();
    let sql_test = enum_member(py, RecordId::CompiledResourceType, "SQL_TEST")?;
    let mut errors: Vec<Bound<'py, PyAny>> = Vec::new();
    for diagnostic in native_field(project, "diagnostics")?.try_iter()? {
        let diagnostic = diagnostic?;
        if native_field(&diagnostic, "is_error")?.is_truthy()?
            && native_field(&diagnostic, "resource_type")?.ne(&sql_test)?
        {
            errors.push(diagnostic);
        }
    }
    let bound = object(
        py,
        source_binding_diagnostics(Some(&keywords(
            py,
            &[
                ("project", project.clone()),
                (
                    "columns",
                    class_field(relations, |context: &PlannerRelationsContext| {
                        &context.source_warehouse_columns
                    })?,
                ),
                (
                    "profile",
                    adapter.call_method0("expression_inference_profile")?,
                ),
                ("selected_keys", selected_keys.clone()),
            ],
        )?))?,
    )?;
    for diagnostic in bound.try_iter()? {
        let diagnostic = diagnostic?;
        if native_field(&diagnostic, "is_error")?.is_truthy()? {
            errors.push(diagnostic);
        }
    }
    let Some(first) = errors.first() else {
        return Ok(());
    };
    let mut lines: Vec<String> = Vec::new();
    for diagnostic in &errors {
        let position = |field: &str| -> PyResult<String> {
            let value = native_field(diagnostic, field)?;
            Ok(if value.is_truthy()? {
                value.str()?.to_string()
            } else {
                "1".to_owned()
            })
        };
        lines.push(format!(
            "error[{}]: {}\n  --> {}:{}:{}",
            native_field(diagnostic, "code")?.str()?,
            native_field(diagnostic, "message")?.str()?,
            native_field(diagnostic, "path")?.str()?,
            position("line")?,
            position("column")?,
        ));
    }
    Err(error_of(
        py,
        ErrorId::PlannerInput,
        &lines.join("\n"),
        &[("code", native_field(first, "code")?)],
    ))
}

fn names_of(entries: &[Bound<'_, PyAny>]) -> PyResult<String> {
    Ok(entries
        .iter()
        .map(|entry| Ok(native_field(entry, "name")?.str()?.to_string()))
        .collect::<PyResult<Vec<String>>>()?
        .join(", "))
}

/// Concurrent microbatch configuration checks, and the warning concurrency complexity earns.
fn check_microbatch_concurrency<'py>(
    project: &Bound<'py, PyAny>,
    adapter: &Bound<'py, PyAny>,
    entries: &[Bound<'py, PyAny>],
    concurrency: i64,
) -> PyResult<Option<Bound<'py, PyAny>>> {
    let py = project.py();
    let skip = planner_type(py, RecordId::PlanAction, "SKIP")?;
    let create = planner_type(py, RecordId::PlanAction, "CREATE_TABLE")?;
    let mut configured: Vec<Bound<'py, PyAny>> = Vec::new();
    for entry in entries {
        if native_field(entry, "batch_concurrency")?.gt(1)?
            && native_field(entry, "action")?.ne(&skip)?
        {
            configured.push(entry.clone());
        }
    }
    let settings = native_field(project, "settings")?;
    if !configured.is_empty() && !native_field(&settings, "microbatch_concurrency")?.is_truthy()? {
        let guidance = object(py, microbatch_guidance())?;
        let prefix: &str = MICROBATCH_CONCURRENCY_MESSAGE;
        return Err(error_of(
            py,
            ErrorId::CompileInput,
            &format!(
                "{prefix}{}; {}",
                names_of(&configured)?,
                guidance.get_item(0)?.str()?
            ),
            &[("help", guidance.get_item(1)?)],
        ));
    }
    if !configured.is_empty()
        && !adapter
            .call_method0("supports_concurrent_microbatch_dml")?
            .is_truthy()?
    {
        return Err(error_of(
            py,
            ErrorId::CompileInput,
            &format!(
                "adapter '{}' does not support concurrent microbatch delete/insert DML; set \
                 batch_concurrency = 1",
                native_field(adapter, "adapter_name")?.str()?
            ),
            &[],
        ));
    }
    let mut concurrent: Vec<Bound<'py, PyAny>> = Vec::new();
    for entry in configured {
        if native_field(&entry, "action")?.ne(&create)? {
            concurrent.push(entry);
        }
    }
    if concurrent.is_empty() || concurrency <= 1 {
        return Ok(None);
    }
    let count = concurrent.len();
    native(
        py,
        RecordId::PlanWarning,
        &[
            ("model_name", none(py)),
            (
                "severity",
                enum_member(py, RecordId::WarningSeverity, "WARNING")?,
            ),
            ("code", string(py, MICROBATCH_COMPLEXITY_CODE)),
            (
                "message",
                string(
                    py,
                    &format!(
                        "Concurrent microbatch execution is enabled for {count} selected \
                             model{}: {}. Concurrent batches may complete out of order and rely \
                             on persistent microbatch history for gap recovery and fingerprint \
                             provenance. State deletion or corruption can require synthetic \
                             reconciliation and weaken version guarantees. Keep \
                             batch_concurrency at 1 unless the runtime benefit justifies this \
                             complexity; see the microbatch concurrency and recovery \
                             documentation.",
                        if count == 1 { "" } else { "s" },
                        names_of(&concurrent)?
                    ),
                ),
            ),
        ],
    )
    .map(Some)
}

/// Build model plan entries from snapshot and per-model resolved actions.
pub(crate) fn plan_entries<'py>(
    request: PlanEntriesRequest<'py>,
) -> PyResult<ModelEntryResults<'py>> {
    let PlanEntriesRequest {
        project,
        adapter,
        scope,
        snapshot,
        relations,
        resolved_models: models,
        cursor_overrides,
        full_refresh,
        build_inputs: inputs,
        concurrency,
    } = request;
    let py = project.py();
    let inputs = if inputs.is_none() {
        Bound::new(py, PlanEntryBuildInputs::new(py, None)?)?.into_any()
    } else {
        inputs
    };
    let build = inputs.cast::<PlanEntryBuildInputs>()?.get();
    let relations_context = relations.cast::<PlannerRelationsContext>()?.get();
    let selected_keys = native_field(&scope, "selected_keys")?;
    check_binding_diagnostics(&project, &adapter, &relations, &selected_keys)?;
    let freshness_blocked = build.source_freshness_blocked_model_names.bind(py).clone();
    let external_blocked = build.external_blocked_model_names.bind(py).clone();
    let start_override = build.start_cursor_override.bind(py).clone();
    let end_override = build.end_cursor_override.bind(py).clone();
    let producers = object(
        py,
        runtime_cursor_producer_names(&scope, &relations_context.source_map.bind(py).clone())?,
    )?;
    let functions_by_name = PyDict::new(py);
    for function in functions(&project)?.try_iter()? {
        let function = function?;
        functions_by_name.set_item(
            sqlbuild_python_project::resource_views::main::function_reads::name::name(&function)?,
            function,
        )?;
    }
    let settings = native_field(&project, "settings")?;
    let scope_data = sqlbuild_python_project::planner_scope::main::scope_data::scope_data(&scope)?;
    let context = Bound::new(
        py,
        ModelPlanContext {
            model_locations: relations_context.model_locations.clone_ref(py),
            models_by_name: sqlbuild_python_project::native_host::models::ModelLookup::Native(
                scope_data.clone(),
            ),
            functions_by_name: functions_by_name.into_any().unbind(),
            seed_locations: relations_context.seed_locations.clone_ref(py),
            function_locations: relations_context.function_locations.clone_ref(py),
            source_map: relations_context.source_read_map.clone_ref(py),
            source_warehouse_columns: relations_context.source_warehouse_columns.clone_ref(py),
            star_exclude_keyword: relations_context.star_exclude_keyword.clone_ref(py),
            runtime_cursor_producer_names: producers.unbind(),
            future_cursor_config: build.future_cursor_config.clone_ref(py),
            start_cursor_config: build.start_cursor_config.clone_ref(py),
            invocation_time: build.invocation_time.clone_ref(py),
        },
    )?
    .into_any();
    let reads = ContextReads::of(
        context,
        adapter.clone(),
        native_field(&project, "external_sql_reference_resolver")?,
    )?;
    let sql_analysis_enabled = native_field(&settings, "sql_analysis")?.is_truthy()?;
    let skip = planner_type(py, RecordId::PlanAction, "SKIP")?;
    let microbatch = enum_member(py, RecordId::IncrementalMode, "MICROBATCH")?;
    let mut entries: Vec<Bound<'py, PyAny>> = Vec::new();
    let mut warnings: Vec<Bound<'py, PyAny>> = Vec::new();
    for key in &scope_data.order {
        let name = &key.1;
        if !scope_data.selected.contains(key) || !models.contains(name)? {
            continue;
        }
        let Some(model) = scope_data.model(py, name) else {
            continue;
        };
        let resolved = models.get_item(name)?;
        let overrides = model_cursor_overrides(
            &model,
            &cursor_overrides,
            (start_override.clone(), end_override.clone()),
        )?;
        let effective_full_refresh = resolve_model_full_refresh(&model, full_refresh)?;
        let (mut entry, entry_warnings) = plan_model_from_change(&ModelPlanRequest {
            model: model.clone(),
            snapshot: snapshot.clone(),
            context: &reads,
            sql_analysis_enabled,
            full_refresh: effective_full_refresh,
            overrides,
            change_result: native_field(&resolved, "change")?,
            protect_full_refresh: !freshness_blocked.contains(name)?
                && !external_blocked.contains(name)?,
        })?;
        let entry_name = native_field(&entry, "name")?;
        if freshness_blocked.contains(&entry_name)? {
            entry = replaced(
                &entry,
                &[
                    ("action", skip.clone()),
                    (
                        "reason",
                        planner_type(py, RecordId::PlanReason, "SOURCE_FRESHNESS_ERROR")?,
                    ),
                ],
            )?;
        }
        if external_blocked.contains(&entry_name)? {
            entry = replaced(
                &entry,
                &[
                    ("action", skip.clone()),
                    (
                        "reason",
                        planner_type(py, RecordId::PlanReason, "EXTERNAL_UPSTREAM_FAILED")?,
                    ),
                ],
            )?;
        }
        if native_field(&entry, "incremental_mode")?.eq(&microbatch)? {
            let policy = native_field(&entry, "unaccounted_partition_policy")?;
            let policy = if policy.is_truthy()? {
                policy
            } else {
                native_field(&settings, "microbatch_unaccounted_partition_policy")?
            };
            entry = replaced(&entry, &[("unaccounted_partition_policy", policy)])?;
            if native_field(&entry, "action")?.ne(&skip)? {
                let limited = apply_configured_microbatch_limits(&entry, &inputs)?;
                entry = limited.get_item(0)?;
                for warning in limited.get_item(1)?.try_iter()? {
                    warnings.push(warning?);
                }
            }
        }
        entries.push(entry);
        for warning in entry_warnings.try_iter()? {
            warnings.push(warning?);
        }
    }
    if let Some(warning) = check_microbatch_concurrency(&project, &adapter, &entries, concurrency)?
    {
        warnings.push(warning);
    }
    Ok(ModelEntryResults { entries, warnings })
}

/// Check if any scope dependency is in the selected keys.
#[pyfunction]
#[pyo3(signature = (*, scope_deps, selected_keys))]
pub(crate) fn scope_overlaps(
    scope_deps: &Bound<'_, PyAny>,
    selected_keys: &Bound<'_, PyAny>,
) -> PyResult<bool> {
    for dependency in scope_deps.try_iter()? {
        if selected_keys.contains(dependency?)? {
            return Ok(true);
        }
    }
    Ok(false)
}
