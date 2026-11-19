//! Maintained models whose live refresh settings drifted, kept ones a replaced upstream reinitialises, and who reads what.

use crate::planner_phases::main::inputs::runtime_reads::runtime_reads;
use crate::planner_phases::models::ModelEntryResults;
use crate::planner_strategy::main::strategy::materialization_of::materialization_of;
use crate::planner_strategy::main::values::text::text;
use crate::planner_warehouse::models::{AdapterRead, PlannerReads, Read};
use pyo3::IntoPyObject;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyBool, PyString, PyTuple};
use sqlbuild_analysis::assembly::project::types::ObjectKey;
use sqlbuild_analysis::maintained_refresh::main::maintained_settings_drift::maintained_settings_drift;
use sqlbuild_analysis::maintained_refresh::main::reinitialised_readers::reinitialised_readers;
use sqlbuild_analysis::maintained_refresh::main::render_target_lag::render_target_lag;
use sqlbuild_analysis::maintained_refresh::models::{
    LiveDynamicTable, MaintainedRead, MaintainedSettings, RefreshLag,
};
use sqlbuild_analysis::planner_scope::constants::MODEL;
use sqlbuild_core::fingerprint_identity::constants::DEFAULT_REFRESH_MODE;
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
use std::collections::{BTreeSet, HashMap};

const MAINTAINED: &str = "maintained";
/// Views pass reads through, so a maintained model reading a view reads the view's inputs.
const VIEW: &str = "view";
const KEEP_MAINTAINED: &str = "keep_maintained";
/// The scheduling state Snowflake lists for a dynamic table whose refreshes are paused.
const SUSPENDED: &str = "SUSPENDED";
/// Actions that recreate the relation, which reinitialises every dynamic table reading it.
const REPLACING_ACTIONS: [&str; 3] = ["create_table", "create_view", "create_maintained"];

/// Plan maintained refresh settings and reinitialised readers, and name the maintained readers of each entry.
pub(crate) fn plan_maintained_refresh<'py>(
    runtime: &Bound<'py, PyAny>,
    execution_scope: &Bound<'py, PyAny>,
    results: ModelEntryResults<'py>,
) -> PyResult<ModelEntryResults<'py>> {
    let py: Python<'py> = runtime.py();
    let scope = scope_data(execution_scope)?;
    let mut upstream: HashMap<&ObjectKey, Vec<String>> = HashMap::new();
    for (key, deps) in &scope.upstream {
        upstream.insert(key, deps.iter().map(|(_, name)| name.clone()).collect());
    }
    let downstream: HashMap<&ObjectKey, &Vec<ObjectKey>> = scope
        .downstream
        .iter()
        .map(|(key, readers)| (key, readers))
        .collect();
    let models = ModelLookup::Native(scope.clone());
    let reads = runtime_reads(runtime)?;
    let mut known = KnownReaders {
        models: &models,
        materializations: HashMap::new(),
        through_view: HashMap::new(),
    };
    let mut entries: Vec<Bound<'py, PyAny>> = Vec::with_capacity(results.entries.len());
    let mut rows: Vec<MaintainedRead> = Vec::with_capacity(results.entries.len());
    for entry in results.entries {
        let key: ObjectKey = native_key(&native_field(&entry, "key")?)?;
        let is_maintained: bool =
            text(&native_field(&entry, "materialization_type")?)? == MAINTAINED;
        let entry = if is_maintained {
            with_refresh_settings(entry, &models, &reads)?
        } else {
            entry
        };
        let readers: Vec<String> = known.maintained_readers(py, &key, &downstream)?;
        let entry = if readers.is_empty() {
            entry
        } else {
            replaced(
                &entry,
                &[("maintained_readers", PyTuple::new(py, readers)?.into_any())],
            )?
        };
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

/// Per-plan lookups: each model's materialization, and the maintained readers behind each view.
struct KnownReaders<'a> {
    models: &'a ModelLookup,
    materializations: HashMap<String, String>,
    through_view: HashMap<String, Vec<String>>,
}

impl KnownReaders<'_> {
    /// The maintained models reading the entry directly or through views, selected or not.
    fn maintained_readers(
        &mut self,
        py: Python<'_>,
        key: &ObjectKey,
        downstream: &HashMap<&ObjectKey, &Vec<ObjectKey>>,
    ) -> PyResult<Vec<String>> {
        let mut found: BTreeSet<String> = BTreeSet::new();
        for reader in downstream
            .get(key)
            .into_iter()
            .flat_map(|readers| readers.iter())
        {
            let (kind, name) = reader;
            if kind != MODEL {
                continue;
            }
            match self.materialization(py, name)?.as_str() {
                MAINTAINED => {
                    found.insert(name.clone());
                }
                VIEW => {
                    if !self.through_view.contains_key(name) {
                        let through: Vec<String> =
                            self.maintained_readers(py, reader, downstream)?;
                        self.through_view.insert(name.clone(), through);
                    }
                    found.extend(self.through_view[name].iter().cloned());
                }
                _ => {}
            }
        }
        Ok(found.into_iter().collect())
    }

    /// A model's materialization, looked up once per plan.
    fn materialization(&mut self, py: Python<'_>, name: &str) -> PyResult<String> {
        if let Some(materialization) = self.materializations.get(name) {
            return Ok(materialization.clone());
        }
        let model = self.models.get(py, name)?;
        let materialization: String = if model.is_none() {
            String::new()
        } else {
            text(&materialization_of(&model)?)?
        };
        self.materializations
            .insert(name.to_owned(), materialization.clone());
        Ok(materialization)
    }
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
    let values = model_values(&model)?;
    let mode: String = match config_text_object(py, &values, "refresh_mode")? {
        Some(mode) => mode.extract()?,
        None => DEFAULT_REFRESH_MODE.to_owned(),
    };
    let mut changes: Vec<(&str, Bound<'py, PyAny>)> = vec![
        (
            "target_lag",
            PyString::new(py, &render_target_lag(desired.lag)).into_any(),
        ),
        ("refresh_mode", PyString::new(py, &mode).into_any()),
        (
            "refresh_warehouse",
            desired.warehouse.as_deref().into_pyobject(py)?.into_any(),
        ),
        (
            "refresh_on_build",
            PyBool::new(
                py,
                config_value(py, &values, "refresh_on_build")?.is_truthy()?,
            )
            .to_owned()
            .into_any(),
        ),
    ];
    if text(&native_field(&entry, "action")?)? == KEEP_MAINTAINED {
        let (action, paused) = live_action(&entry, &desired, reads)?;
        if let Some((action, reason)) = action {
            changes.push(("action", enum_member(py, RecordId::PlanAction, action)?));
            changes.push(("reason", enum_member(py, RecordId::PlanReason, reason)?));
        }
        changes.push((
            "maintained_paused",
            PyBool::new(py, paused).to_owned().into_any(),
        ));
    }
    replaced(&entry, &changes)
}

/// The action and reason replacing a kept model's, `None` when its listing matches or is unknown, and whether it is suspended.
fn live_action(
    entry: &Bound<'_, PyAny>,
    desired: &MaintainedSettings,
    reads: &Bound<'_, PlannerReads>,
) -> PyResult<(Option<(&'static str, &'static str)>, bool)> {
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
        Read::Unknown(_) => return Ok((None, false)),
        Read::Known(listed) => listed,
    };
    if listed.is_none() {
        return Ok((Some(("CREATE_MAINTAINED", "FIRST_RUN")), false));
    }
    let (target_lag, warehouse, state): (String, String, String) = listed.extract()?;
    let live = LiveDynamicTable {
        target_lag,
        warehouse,
    };
    let action =
        maintained_settings_drift(desired, &live).then_some(("ALTER_MAINTAINED", "CONFIG_CHANGED"));
    Ok((action, state == SUSPENDED))
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
