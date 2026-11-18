//! Maintained models whose live refresh settings drifted, and kept ones a replaced upstream reinitialises.

use crate::planner_phases::main::inputs::runtime_reads::runtime_reads;
use crate::planner_phases::models::ModelEntryResults;
use crate::planner_strategy::main::values::text::text;
use crate::planner_warehouse::models::{AdapterRead, PlannerReads, Read};
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::PyString;
use sqlbuild_analysis::maintained_refresh::main::maintained_settings_drift::maintained_settings_drift;
use sqlbuild_analysis::maintained_refresh::main::reinitialised_readers::reinitialised_readers;
use sqlbuild_analysis::maintained_refresh::main::render_target_lag::render_target_lag;
use sqlbuild_analysis::maintained_refresh::models::{
    LiveDynamicTable, MaintainedRead, MaintainedSettings, RefreshLag,
};
use sqlbuild_model_config::model_validation::main::fixed_duration_seconds::fixed_duration_seconds;
use sqlbuild_python_base::records::main::construct::enum_member::enum_member;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::main::fields::replaced::replaced;
use sqlbuild_python_base::records::types::RecordId;
use sqlbuild_python_front::model_config::main::config::config_text_object::config_text_object;
use sqlbuild_python_front::model_config::main::config::config_value::config_value;
use sqlbuild_python_front::model_config::main::config::model_values::model_values;
use sqlbuild_python_project::graph::main::nodes::native_key::native_key;
use sqlbuild_python_project::native_host::models::ModelLookup;
use sqlbuild_python_project::planner_scope::main::scope_data::scope_data;
use std::collections::HashMap;

const MAINTAINED: &str = "maintained";
const KEEP_MAINTAINED: &str = "keep_maintained";
/// Actions that recreate the relation, which reinitialises every dynamic table reading it.
const REPLACING_ACTIONS: [&str; 3] = ["create_table", "create_view", "create_maintained"];

/// Alter drifted maintained models, set their `target_lag`, and mark kept readers of a replaced upstream.
pub(crate) fn plan_maintained_refresh<'py>(
    runtime: &Bound<'py, PyAny>,
    execution_scope: &Bound<'py, PyAny>,
    results: ModelEntryResults<'py>,
) -> PyResult<ModelEntryResults<'py>> {
    let mut maintained: Vec<bool> = Vec::with_capacity(results.entries.len());
    for entry in &results.entries {
        maintained.push(text(&native_field(entry, "materialization_type")?)? == MAINTAINED);
    }
    if !maintained.contains(&true) {
        return Ok(results);
    }
    let py: Python<'py> = runtime.py();
    let scope = scope_data(execution_scope)?;
    let mut upstream: HashMap<&(String, String), Vec<String>> = HashMap::new();
    for (key, deps) in &scope.upstream {
        upstream.insert(key, deps.iter().map(|(_, name)| name.clone()).collect());
    }
    let models = ModelLookup::Native(scope.clone());
    let reads = runtime_reads(runtime)?;
    let mut entries: Vec<Bound<'py, PyAny>> = Vec::with_capacity(results.entries.len());
    let mut rows: Vec<MaintainedRead> = Vec::with_capacity(results.entries.len());
    for (entry, is_maintained) in results.entries.into_iter().zip(maintained) {
        let entry = if is_maintained {
            with_refresh_settings(entry, &models, &reads)?
        } else {
            entry
        };
        let key = native_key(&native_field(&entry, "key")?)?;
        let action: String = text(&native_field(&entry, "action")?)?;
        rows.push(MaintainedRead {
            name: key.1.clone(),
            maintained: is_maintained,
            replaced: REPLACING_ACTIONS.contains(&action.as_str()),
            reads: upstream.remove(&key).unwrap_or_default(),
        });
        entries.push(entry);
    }
    let readers: HashMap<String, String> = reinitialised_readers(&rows).into_iter().collect();
    for (entry, row) in entries.iter_mut().zip(&rows) {
        if let Some(upstream) = readers.get(&row.name) {
            *entry = replaced(
                entry,
                &[("reinitialised_by", PyString::new(py, upstream).into_any())],
            )?;
        }
    }
    Ok(ModelEntryResults {
        entries,
        warnings: results.warnings,
    })
}

/// The entry with its `target_lag`; a kept one alters on drift, or creates when no dynamic table holds its name.
fn with_refresh_settings<'py>(
    entry: Bound<'py, PyAny>,
    models: &ModelLookup,
    reads: &Bound<'py, PlannerReads>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = entry.py();
    let model = models.get(py, &native_field(&entry, "name")?.extract::<String>()?)?;
    let Some(desired) = declared_settings(&model)? else {
        return Ok(entry);
    };
    let mut changes: Vec<(&str, Bound<'py, PyAny>)> = vec![(
        "target_lag",
        PyString::new(py, &render_target_lag(desired.lag)).into_any(),
    )];
    if text(&native_field(&entry, "action")?)? == KEEP_MAINTAINED {
        if let Some((action, reason)) = live_action(&entry, &desired, reads)? {
            changes.push(("action", enum_member(py, RecordId::PlanAction, action)?));
            changes.push(("reason", enum_member(py, RecordId::PlanReason, reason)?));
        }
    }
    replaced(&entry, &changes)
}

/// The action and reason replacing a kept model's, or `None` when its listed settings match or are unknown.
fn live_action(
    entry: &Bound<'_, PyAny>,
    desired: &MaintainedSettings,
    reads: &Bound<'_, PlannerReads>,
) -> PyResult<Option<(&'static str, &'static str)>> {
    let destination = native_field(entry, "destination")?;
    let read = reads.get().reads().adapter_read(
        entry.py(),
        AdapterRead::MaintainedRefresh([
            native_field(&destination, "database")?,
            native_field(&destination, "schema")?,
            native_field(&destination, "name")?,
        ]),
    )?;
    let listed = match read {
        Read::Unknown(_) => return Ok(None),
        Read::Known(listed) => listed,
    };
    if listed.is_none() {
        return Ok(Some(("CREATE_MAINTAINED", "FIRST_RUN")));
    }
    let (target_lag, warehouse): (String, String) = listed.extract()?;
    let live = LiveDynamicTable {
        target_lag,
        warehouse,
    };
    Ok(maintained_settings_drift(desired, &live).then_some(("ALTER_MAINTAINED", "CONFIG_CHANGED")))
}

/// The model's declared lag and refresh warehouse, or `None` when its lag does not read.
fn declared_settings(model: &Bound<'_, PyAny>) -> PyResult<Option<MaintainedSettings>> {
    let py = model.py();
    let values = model_values(model)?;
    let refresh = config_value(py, &values, "refresh")?;
    let lag: RefreshLag = if refresh.is_none() {
        RefreshLag::Downstream
    } else {
        let text: String = refresh.get_item("lag")?.extract()?;
        let Some(seconds) = fixed_duration_seconds(&text) else {
            return Ok(None);
        };
        match u64::try_from(seconds) {
            Ok(seconds) => RefreshLag::Seconds(seconds),
            Err(_) => return Ok(None),
        }
    };
    let warehouse: Option<String> = config_text_object(py, &values, "refresh_warehouse")?
        .map(|warehouse| warehouse.extract())
        .transpose()?;
    Ok(Some(MaintainedSettings { lag, warehouse }))
}
