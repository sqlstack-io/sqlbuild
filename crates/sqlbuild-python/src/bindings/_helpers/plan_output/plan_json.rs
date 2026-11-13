//! The JSON plan document, read from the planner's output by attribute.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyModule, PyModuleMethods, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods, PyList, PyListMethods, PyString};
use pyo3::{pyfunction, wrap_pyfunction};
use sqlbuild_core::plan_display::constants::SYNCED_COPY_KEY;

use crate::bindings::_helpers::boundary::python_calls::KeywordArguments;
use crate::bindings::_helpers::compile_outputs::json_report::indented_json;
use crate::bindings::_helpers::cursor::scalars::sentinel_to_token;
use crate::bindings::_helpers::plan_output::cursor_details::{
    CursorDetails, cursor_details, owner,
};
use crate::bindings::_helpers::plan_output::plan_text::{model_config_diff, model_query_diff};
use crate::bindings::_helpers::plan_output::python_identity::{
    definition_source_text, dependency_source_text,
};
use crate::bindings::_helpers::plan_output::safety::{future_cursor_safety, maximum_start_safety};
use crate::bindings::_helpers::plan_output::values::{
    any, dict, enum_value, iso, items, list, mapped, optional_enum_value, relation_name, text,
};
use crate::bindings::_helpers::records::fields::field as native_field;

const MICROBATCH_MODE: &str = "microbatch";
const RENAMED_REASON: &str = "renamed";
const RENAMED_DECISION: &str = "renamed";
const VIEW_MATERIALIZATION: &str = "view";
const CHANGED_IDENTITY: &str = "changed";

fn token<'py>(bound: Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    Ok(PyString::new(bound.py(), &sentinel_to_token(&bound)?).into_any())
}

fn token_bounds<'py>(bounds: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    Ok(dict(
        bounds.py(),
        vec![
            ("start", token(native_field(bounds, "start")?)?),
            ("end", token(native_field(bounds, "end")?)?),
        ],
    )?
    .into_any())
}

/// `_serialize_cursor_details`: requested, resolved and deferred cursor-plan state.
fn cursor_json<'py>(py: Python<'py>, details: &CursorDetails<'py>) -> PyResult<Bound<'py, PyAny>> {
    let resolved: Bound<'py, PyAny> = match &details.resolved_bounds {
        Some(bounds) => token_bounds(bounds)?,
        None => py.None().into_bound(py),
    };
    Ok(dict(
        py,
        vec![
            (
                "requested_bounds",
                dict(
                    py,
                    vec![
                        ("start", any(py, details.requested_start.clone())?),
                        ("end", any(py, details.requested_end.clone())?),
                    ],
                )?
                .into_any(),
            ),
            ("bounds_owner", any(py, owner(details))?),
            ("resolution_status", any(py, details.resolution_status)?),
            ("resolved_bounds", resolved),
            ("declared_grain", any(py, details.declared_grain.clone())?),
            ("effective_grain", any(py, details.effective_grain.clone())?),
            (
                "declared_batch_size",
                any(py, details.declared_batch_size.clone())?,
            ),
            (
                "effective_batch_size",
                any(py, details.effective_batch_size.clone())?,
            ),
            ("planned_batch_count", any(py, details.planned_batch_count)?),
            ("empty_inputs", list(&details.empty_inputs)?),
            (
                "waiting_on_empty_inputs",
                any(py, details.waiting_on_empty_inputs)?,
            ),
        ],
    )?
    .into_any())
}

fn identity_status(entry: &Bound<'_, PyAny>) -> PyResult<&'static str> {
    let expected: Bound<'_, PyAny> = native_field(entry, "fingerprint_version_hash")?;
    let built: Bound<'_, PyAny> = native_field(entry, "previous_version_hash")?;
    Ok(if expected.is_none() {
        "unknown"
    } else if built.is_none() {
        "missing"
    } else if built.eq(&expected)? {
        "current"
    } else {
        "stale"
    })
}

/// The safety evidence of the microbatch range when it has some, else of the cursor bounds.
fn evidence<'py>(entry: &Bound<'py, PyAny>, field: &str) -> PyResult<Bound<'py, PyAny>> {
    let microbatch_range: Bound<'py, PyAny> = native_field(entry, "microbatch_range")?;
    if !microbatch_range.is_none() {
        let found: Bound<'py, PyAny> = native_field(&microbatch_range, field)?;
        if !found.is_none() {
            return Ok(found);
        }
    }
    let bounds: Bound<'py, PyAny> = native_field(entry, "cursor_bounds")?;
    if bounds.is_none() {
        return Ok(bounds);
    }
    native_field(&bounds, field)
}

fn microbatch_state<'py>(entry: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = entry.py();
    let state: Bound<'py, PyDict> = dict(
        py,
        vec![
            ("completion_tracking", any(py, "universal")?),
            (
                "batch_concurrency",
                native_field(entry, "batch_concurrency")?,
            ),
            (
                "unaccounted_partition_policy",
                native_field(entry, "unaccounted_partition_policy")?,
            ),
            ("reconciliation", any(py, "runtime")?),
        ],
    )?;
    let limit: Bound<'py, PyAny> = native_field(entry, "microbatch_limit")?;
    if !limit.is_none() {
        state.set_item("limit", limit)?;
        state.set_item("count", native_field(entry, "microbatch_limit_count")?)?;
        state.set_item(
            "action",
            optional_enum_value(&native_field(entry, "microbatch_limit_action")?)?,
        )?;
        state.set_item("warning", native_field(entry, "microbatch_limit_warning")?)?;
    }
    Ok(state.into_any())
}

fn model_json<'py>(entry: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = entry.py();
    let model: Bound<'py, PyDict> = dict(
        py,
        vec![
            ("name", native_field(entry, "name")?),
            (
                "relative_path",
                native_field(entry, "relative_path")?.str()?.into_any(),
            ),
            (
                "materialization_type",
                enum_value(entry, "materialization_type")?,
            ),
            ("action", enum_value(entry, "action")?),
            ("reason", enum_value(entry, "reason")?),
            (
                "expected_version_hash",
                native_field(entry, "fingerprint_version_hash")?,
            ),
            (
                "built_version_hash",
                native_field(entry, "previous_version_hash")?,
            ),
            (
                "built_version_present",
                any(py, !native_field(entry, "previous_version_hash")?.is_none())?,
            ),
            ("identity_status", any(py, identity_status(entry)?)?),
        ],
    )?;
    for key in ["incremental_strategy", "incremental_mode"] {
        let value: Bound<'py, PyAny> = native_field(entry, key)?;
        if !value.is_none() {
            model.set_item(key, value)?;
        }
    }
    if native_field(entry, "incremental_mode")?.eq(MICROBATCH_MODE)? {
        model.set_item("microbatch_state", microbatch_state(entry)?)?;
    }
    for key in ["cursor_column", "cursor_type"] {
        let value: Bound<'py, PyAny> = native_field(entry, key)?;
        if !value.is_none() {
            model.set_item(key, value)?;
        }
    }
    let bounds: Bound<'py, PyAny> = native_field(entry, "cursor_bounds")?;
    if !bounds.is_none() {
        model.set_item("cursor_bounds", token_bounds(&bounds)?)?;
    }
    let future: Bound<'py, PyAny> = future_cursor_safety(&evidence(entry, "future_safety")?)?;
    if !future.is_none() {
        model.set_item("future_cursor_safety", future)?;
    }
    let maximum: Bound<'py, PyAny> =
        maximum_start_safety(&evidence(entry, "maximum_start_safety")?)?;
    if !maximum.is_none() {
        model.set_item("maximum_start_safety", maximum)?;
    }
    if let Some(details) = cursor_details(entry)? {
        model.set_item("cursor", cursor_json(py, &details)?)?;
    }
    let backfill: Bound<'py, PyAny> = native_field(entry, "backfill")?;
    model.set_item(
        "backfill",
        dict(
            py,
            vec![
                ("action", enum_value(&backfill, "action")?),
                ("duration", native_field(&backfill, "duration")?),
            ],
        )?,
    )?;
    if native_field(entry, "reason")?.eq(RENAMED_REASON)?
        && native_field(entry, "query_changed")?.is_truthy()?
    {
        model.set_item("query_changed", true)?;
    }
    let changed_functions: Bound<'py, PyAny> = native_field(entry, "changed_functions")?;
    if changed_functions.is_truthy()? {
        model.set_item("changed_functions", list(&changed_functions)?)?;
    }
    let qualified: Bound<'py, PyAny> =
        native_field(&native_field(entry, "destination")?, "qualified_name")?;
    if !qualified.is_none() {
        model.set_item("qualified_name", qualified)?;
    }
    let reason: String = text(&native_field(&native_field(entry, "reason")?, "value")?)?;
    if let Some(diff) = model_query_diff(entry, &reason)? {
        model.set_item("query_diff", diff)?;
    }
    if let Some(diff) = model_config_diff(entry, &reason)? {
        model.set_item("config_diff", diff)?;
    }
    let findings: Bound<'py, PyAny> = native_field(entry, "schema_findings")?;
    if findings.is_truthy()? {
        model.set_item(
            "schema_findings",
            mapped(&findings, |finding| {
                Ok(dict(
                    py,
                    vec![
                        ("kind", enum_value(finding, "kind")?),
                        ("column", native_field(finding, "column_name")?),
                        ("expected_type", native_field(finding, "expected_type")?),
                        ("actual_type", native_field(finding, "actual_type")?),
                    ],
                )?
                .into_any())
            })?,
        )?;
    }
    let hints: Bound<'py, PyAny> = native_field(entry, "column_rename_hints")?;
    if hints.is_truthy()? {
        model.set_item(
            "column_rename_hints",
            mapped(&hints, |hint| {
                Ok(dict(
                    py,
                    vec![
                        ("column", native_field(hint, "added_column")?),
                        (
                            "candidates",
                            list(&native_field(hint, "candidate_columns")?)?,
                        ),
                        ("identical", native_field(hint, "identical")?),
                        ("hint", native_field(hint, "message")?),
                    ],
                )?
                .into_any())
            })?,
        )?;
    }
    Ok(model.into_any())
}

fn with_qualified_name<'py>(
    item: Bound<'py, PyDict>,
    entry: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let qualified: Bound<'py, PyAny> =
        native_field(&native_field(entry, "destination")?, "qualified_name")?;
    if !qualified.is_none() {
        item.set_item("qualified_name", qualified)?;
    }
    Ok(item.into_any())
}

fn seed_json<'py>(entry: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = entry.py();
    with_qualified_name(
        dict(
            py,
            vec![
                ("name", native_field(entry, "name")?),
                ("reason", enum_value(entry, "reason")?),
            ],
        )?,
        entry,
    )
}

fn source_load_json<'py>(entry: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = entry.py();
    let load: Bound<'py, PyDict> = dict(
        py,
        vec![
            ("name", native_field(entry, "name")?),
            ("loader", native_field(entry, "loader")?),
            ("kind", enum_value(entry, "resource_kind")?),
            ("target", native_field(entry, "destination")?),
            ("is_reload", native_field(entry, "is_reload")?),
        ],
    )?;
    let strategy: Bound<'py, PyAny> = native_field(entry, "write_strategy")?;
    if !strategy.is_none() {
        load.set_item("write_strategy", native_field(&strategy, "value")?)?;
    }
    let cursor: Bound<'py, PyAny> = native_field(entry, "cursor_column")?;
    if !cursor.is_none() {
        load.set_item("cursor_column", cursor)?;
    }
    let unique_key: Bound<'py, PyAny> = native_field(entry, "unique_key")?;
    if unique_key.is_truthy()? {
        load.set_item("unique_key", unique_key)?;
    }
    Ok(load.into_any())
}

fn function_json<'py>(entry: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = entry.py();
    let columns: Bound<'py, PyAny> = native_field(entry, "return_columns")?;
    with_qualified_name(
        dict(
            py,
            vec![
                ("name", native_field(entry, "name")?),
                (
                    "relative_path",
                    native_field(entry, "relative_path")?.str()?.into_any(),
                ),
                ("language", enum_value(entry, "language")?),
                (
                    "return_kind",
                    any(
                        py,
                        if columns.is_truthy()? {
                            "table"
                        } else {
                            "scalar"
                        },
                    )?,
                ),
                ("returns", native_field(entry, "returns")?),
                (
                    "return_columns",
                    mapped(&columns, |column| {
                        Ok(dict(
                            py,
                            vec![
                                ("name", native_field(column, "name")?),
                                ("type", native_field(column, "type")?),
                            ],
                        )?
                        .into_any())
                    })?,
                ),
            ],
        )?,
        entry,
    )
}

fn warning_json<'py>(warning: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = warning.py();
    let item: Bound<'py, PyDict> = dict(
        py,
        vec![
            ("severity", enum_value(warning, "severity")?),
            ("message", native_field(warning, "message")?),
        ],
    )?;
    for key in ["model_name", "code"] {
        let value: Bound<'py, PyAny> = native_field(warning, key)?;
        if !value.is_none() {
            item.set_item(key, value)?;
        }
    }
    Ok(item.into_any())
}

/// `difflib.unified_diff` between two texts, each line without its newline.
pub(crate) fn unified_diff<'py>(
    py: Python<'py>,
    previous: &str,
    current: &str,
) -> PyResult<Vec<String>> {
    let lines = |text: &str| -> PyResult<Bound<'py, PyAny>> {
        PyString::new(py, text).call_method1("splitlines", (true,))
    };
    let keywords: Bound<'py, PyDict> = PyDict::new(py);
    keywords.set_item("fromfile", "previous")?;
    keywords.set_item("tofile", "current")?;
    let diff: Bound<'py, PyAny> = py
        .import("difflib")?
        .getattr("unified_diff")?
        .call((lines(previous)?, lines(current)?), Some(&keywords))?;
    let mut rendered: Vec<String> = Vec::new();
    for line in diff.try_iter()? {
        let line: String = line?.extract()?;
        rendered.push(line.trim_end_matches('\n').to_owned());
    }
    Ok(rendered)
}

fn identity_diff<'py>(entry: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyDict>> {
    let py: Python<'py> = entry.py();
    let result: Bound<'py, PyDict> = PyDict::new(py);
    if !native_field(entry, "identity_status")?.eq(CHANGED_IDENTITY)? {
        return Ok(result);
    }
    let pairs = [
        (
            "source_diff",
            definition_source_text(&native_field(entry, "previous_definition_json")?)?,
            definition_source_text(&native_field(entry, "current_definition_json")?)?,
        ),
        (
            "dependency_diff",
            dependency_source_text(&native_field(entry, "previous_metadata_json")?)?,
            dependency_source_text(&native_field(entry, "current_metadata_json")?)?,
        ),
    ];
    for (key, previous, current) in pairs {
        if let (Some(previous), Some(current)) = (previous, current)
            && previous != current
        {
            let diff: Vec<String> = unified_diff(py, &previous, &current)?;
            if !diff.is_empty() {
                result.set_item(key, diff)?;
            }
        }
    }
    Ok(result)
}

fn python_entry_json<'py>(entry: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = entry.py();
    let item: Bound<'py, PyDict> = dict(
        py,
        vec![
            ("name", native_field(entry, "name")?),
            ("kind", enum_value(entry, "kind")?),
            ("phase", enum_value(entry, "phase")?),
            ("identity_status", enum_value(entry, "identity_status")?),
        ],
    )?;
    let diff: Bound<'py, PyDict> = identity_diff(entry)?;
    if !diff.is_empty() {
        item.set_item("identity_diff", diff)?;
    }
    Ok(item.into_any())
}

/// One provider usage: provider, `(kind, name, parameter)` consumer, annotation class and module.
struct Usage<'py> {
    provider: String,
    consumer: (String, String, String),
    class_name: Bound<'py, PyAny>,
    module: Bound<'py, PyAny>,
}

fn providers_json<'py>(
    plan: &Bound<'py, PyAny>,
    python_entries: &[Bound<'py, PyAny>],
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = plan.py();
    let mut usages: Vec<Usage<'py>> = Vec::new();
    for usage in items(&native_field(plan, "provider_usages")?)? {
        usages.push(Usage {
            provider: text(&native_field(&usage, "provider_name")?)?,
            consumer: (
                text(&native_field(&usage, "consumer_kind")?)?,
                text(&native_field(&usage, "consumer_name")?)?,
                text(&native_field(&usage, "parameter_name")?)?,
            ),
            class_name: native_field(&usage, "annotation_class_name")?,
            module: native_field(&usage, "annotation_module")?,
        });
    }
    for entry in python_entries {
        let kind: String = text(&enum_value(entry, "kind")?)?;
        let name: String = text(&native_field(entry, "name")?)?;
        for usage in items(&native_field(entry, "provider_usages")?)? {
            usages.push(Usage {
                provider: text(&native_field(&usage, "provider_name")?)?,
                consumer: (
                    kind.clone(),
                    name.clone(),
                    text(&native_field(&usage, "parameter_name")?)?,
                ),
                class_name: native_field(&usage, "annotation_class_name")?,
                module: native_field(&usage, "annotation_module")?,
            });
        }
    }
    usages.sort_by(|left, right| {
        (&left.provider, &left.consumer).cmp(&(&right.provider, &right.consumer))
    });
    let providers: Bound<'py, PyList> = PyList::empty(py);
    let mut current: Option<(String, Bound<'py, PyList>)> = None;
    for Usage {
        provider,
        consumer: (kind, name, parameter),
        class_name,
        module,
    } in usages
    {
        if current.as_ref().is_none_or(|(open, _)| *open != provider) {
            let used_by: Bound<'py, PyList> = PyList::empty(py);
            providers.append(dict(
                py,
                vec![
                    ("name", any(py, provider.as_str())?),
                    ("used_by", used_by.clone().into_any()),
                ],
            )?)?;
            current = Some((provider, used_by));
        }
        let usage: Bound<'py, PyDict> = dict(
            py,
            vec![
                ("kind", any(py, kind)?),
                ("name", any(py, name)?),
                ("parameter", any(py, parameter)?),
            ],
        )?;
        if !class_name.is_none() || !module.is_none() {
            usage.set_item(
                "annotation",
                dict(py, vec![("class_name", class_name), ("module", module)])?,
            )?;
        }
        if let Some((_, used_by)) = &current {
            used_by.append(usage)?;
        }
    }
    Ok(providers.into_any())
}

fn old_name_json<'py>(entry: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = entry.py();
    Ok(dict(
        py,
        vec![
            ("model", native_field(entry, "model_name")?),
            ("action", enum_value(entry, "action")?),
            ("view", relation_name(&native_field(entry, "origin")?)?),
            (
                "reads",
                relation_name(&native_field(entry, "destination")?)?,
            ),
            ("expires_at", iso(&native_field(entry, "expires_at")?)?),
            (
                "column_aliases",
                py.import("builtins")?
                    .getattr("dict")?
                    .call1((native_field(entry, "column_aliases")?,))?,
            ),
            ("grants_copied", native_field(entry, "grants_copied")?),
            ("reason", native_field(entry, "reason")?),
        ],
    )?
    .into_any())
}

fn migrations_json<'py>(plan: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = plan.py();
    let old_names: Bound<'py, PyDict> = PyDict::new(py);
    for entry in items(&native_field(plan, "old_name_view_entries")?)? {
        old_names.set_item(native_field(&entry, "model_name")?, entry)?;
    }
    let materializations: Bound<'py, PyDict> = PyDict::new(py);
    for entry in items(&native_field(plan, "model_entries")?)? {
        materializations.set_item(
            native_field(&entry, "name")?,
            native_field(&entry, "materialization_type")?.str()?,
        )?;
    }
    mapped(&native_field(plan, "migration_entries")?, |entry| {
        let model: Bound<'py, PyAny> = native_field(entry, "model_name")?;
        let transfer: Bound<'py, PyAny> = native_field(entry, "transfer")?;
        let completed_at: Bound<'py, PyAny> = native_field(entry, "completed_at")?;
        let transfer_value: Bound<'py, PyAny> = if !transfer.is_none() {
            native_field(&transfer, "value")?
        } else if !native_field(entry, "decision")?.eq(RENAMED_DECISION)? || !completed_at.is_none()
        {
            py.None().into_bound(py)
        } else {
            let view: bool = materializations
                .get_item(&model)?
                .is_some_and(|kind| kind.eq(VIEW_MATERIALIZATION).unwrap_or(false));
            any(py, if view { "recreate" } else { "rebuild" })?
        };
        let old_name: Bound<'py, PyAny> = match old_names.get_item(&model)? {
            Some(old_name) => old_name_json(&old_name)?,
            None => py.None().into_bound(py),
        };
        Ok(dict(
            py,
            vec![
                ("kind", any(py, "model_migration")?),
                ("model", model.clone()),
                ("discovery", enum_value(entry, "discovery")?),
                ("decision", enum_value(entry, "decision")?),
                ("compatibility", enum_value(entry, "compatibility")?),
                (
                    "compatibility_findings",
                    list(&native_field(entry, "compatibility_findings")?)?,
                ),
                ("origin_model", native_field(entry, "origin_model")?),
                ("origin", relation_name(&native_field(entry, "origin")?)?),
                (
                    "destination",
                    relation_name(&native_field(entry, "destination")?)?,
                ),
                ("target", native_field(entry, "target_name")?),
                (
                    "origin_version_hash",
                    native_field(entry, "origin_version_hash")?,
                ),
                ("transfer", transfer_value),
                (
                    "transfer_fallback",
                    optional_enum_value(&native_field(entry, "transfer_fallback")?)?,
                ),
                (
                    "storage_transition",
                    native_field(entry, "storage_transition")?,
                ),
                (
                    "promotion",
                    optional_enum_value(&native_field(entry, "promotion")?)?,
                ),
                ("completed_at", iso(&completed_at)?),
                ("old_name", old_name),
            ],
        )?
        .into_any())
    })
}

fn column_migrations_json<'py>(plan: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = plan.py();
    mapped(&native_field(plan, "column_migration_entries")?, |entry| {
        Ok(dict(
            py,
            vec![
                ("kind", any(py, "column_migration")?),
                ("model", native_field(entry, "model_name")?),
                (
                    "relation",
                    relation_name(&native_field(entry, "destination")?)?,
                ),
                ("origin_column", native_field(entry, "origin_column")?),
                (
                    "destination_column",
                    native_field(entry, "destination_column")?,
                ),
                ("discovery", enum_value(entry, "discovery")?),
                ("decision", enum_value(entry, "decision")?),
                ("target", native_field(entry, "target_name")?),
                ("completed_at", iso(&native_field(entry, "completed_at")?)?),
            ],
        )?
        .into_any())
    })
}

fn retention_json<'py>(plan: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = plan.py();
    mapped(&native_field(plan, "retention_entries")?, |entry| {
        let request: Bound<'py, PyAny> = native_field(entry, "request")?;
        Ok(dict(
            py,
            vec![
                ("scope", enum_value(&request, "scope")?),
                ("database", native_field(&request, "database")?),
                ("schema", native_field(&request, "schema")?),
                ("name", native_field(&request, "name")?),
                ("models", native_field(entry, "model_names")?),
                ("desired_days", native_field(&request, "desired_days")?),
                ("actual_days", native_field(entry, "actual_days")?),
                ("effective_days", native_field(entry, "effective_days")?),
                ("source", native_field(entry, "source")?),
                ("direction", enum_value(entry, "direction")?),
                ("phase", enum_value(entry, "phase")?),
                ("statements", native_field(entry, "statements")?),
                (
                    "irreversible_warning",
                    native_field(entry, "irreversible_warning")?,
                ),
                ("decrease_policy", native_field(entry, "decrease_policy")?),
            ],
        )?
        .into_any())
    })
}

fn table_type_json<'py>(plan: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = plan.py();
    mapped(&native_field(plan, "table_type_entries")?, |entry| {
        Ok(dict(
            py,
            vec![
                ("kind", any(py, "table_type_conversion")?),
                ("model", native_field(entry, "model_name")?),
                ("desired_type", native_field(entry, "desired_type")?),
                ("actual_type", native_field(entry, "actual_type")?),
                ("source", native_field(entry, "source")?),
                ("downgrade", native_field(entry, "downgrade")?),
                ("downgrade_policy", native_field(entry, "downgrade_policy")?),
                (
                    "irreversible_warning",
                    native_field(entry, "irreversible_warning")?,
                ),
            ],
        )?
        .into_any())
    })
}

/// The plan as one JSON document, `json.dumps(indent=2)`.
#[pyfunction]
#[pyo3(signature = (**options))]
pub(crate) fn format_plan_json(
    py: Python<'_>,
    options: Option<Bound<'_, PyDict>>,
) -> PyResult<String> {
    let arguments: KeywordArguments<'_> = KeywordArguments::read(
        py,
        "format_plan_json",
        options,
        (&["plan"], &["python_plan_entries", SYNCED_COPY_KEY]),
    )?;
    let plan: Bound<'_, PyAny> = arguments.value("plan")?;
    let python_entries: Vec<Bound<'_, PyAny>> = if arguments.passed("python_plan_entries")? {
        items(&arguments.value("python_plan_entries")?)?
    } else {
        Vec::new()
    };
    let length = |field: &str| -> PyResult<usize> { native_field(&plan, field)?.len() };
    let python_nodes: Vec<Bound<'_, PyAny>> = python_entries
        .iter()
        .map(python_entry_json)
        .collect::<PyResult<_>>()?;
    let result: Bound<'_, PyDict> = dict(
        py,
        vec![
            (
                "selected_count",
                any(
                    py,
                    length("model_entries")?
                        + length("seed_entries")?
                        + length("function_entries")?,
                )?,
            ),
            (
                "source_load_count",
                any(py, length("source_load_entries")?)?,
            ),
            ("python_node_count", any(py, python_nodes.len())?),
            (
                "models",
                mapped(&native_field(&plan, "model_entries")?, model_json)?,
            ),
            (
                "seeds",
                mapped(&native_field(&plan, "seed_entries")?, seed_json)?,
            ),
            (
                "source_loads",
                mapped(
                    &native_field(&plan, "source_load_entries")?,
                    source_load_json,
                )?,
            ),
            (
                "functions",
                mapped(&native_field(&plan, "function_entries")?, function_json)?,
            ),
            ("python_nodes", PyList::new(py, python_nodes)?.into_any()),
            ("providers", providers_json(&plan, &python_entries)?),
            (
                "warnings",
                mapped(&native_field(&plan, "warnings")?, warning_json)?,
            ),
            ("retention", retention_json(&plan)?),
            ("table_type_conversions", table_type_json(&plan)?),
            ("migrations", migrations_json(&plan)?),
            ("column_migrations", column_migrations_json(&plan)?),
            (
                "old_names",
                mapped(
                    &native_field(&plan, "old_name_view_entries")?,
                    old_name_json,
                )?,
            ),
        ],
    )?;
    let metadata: Bound<'_, PyAny> = native_field(&plan, "metadata")?;
    if metadata.is_truthy()? {
        result.set_item("metadata", metadata)?;
    }
    let synced_copy: Bound<'_, PyAny> = arguments.value(SYNCED_COPY_KEY)?;
    if !synced_copy.is_none() {
        result.set_item(SYNCED_COPY_KEY, synced_copy)?;
    }
    indented_json(result.as_any())
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(format_plan_json, module)?)
}
