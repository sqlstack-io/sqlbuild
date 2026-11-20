//! Preserve event and declared-origin precedence at the inspection boundary.

use crate::planner_migrations::_helpers::planning::location_relation::location_relation;

use crate::planner_migrations::_helpers::column::column_objects::config;
use crate::planner_migrations::models::ModelMigrationDeclaration;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use pyo3::types::{PyDict, PyDictMethods, PyList, PyListMethods, PyTuple};
use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_front::state_rows::main::migrations::newest_move::newest_move;
use sqlbuild_python_project::compiled_views::main::compiled_models::name_object::name_object;

pub(crate) fn identity(item: &Bound<'_, PyAny>) -> PyResult<bool> {
    let materialized = config(item, "materialized")?;
    Ok(materialized.eq("table")? || materialized.eq("view")?)
}

pub(crate) fn history(item: &Bound<'_, PyAny>) -> PyResult<bool> {
    let materialized = config(item, "materialized")?;
    Ok(materialized.eq("incremental")? || materialized.eq("snapshot")?)
}

pub(crate) fn newest<'py>(
    events: &Bound<'py, PyAny>,
    location: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let relation = location_relation(location)?;
    newest_move(events, &relation)
}

pub(crate) fn relation<'py>(location: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    location_relation(location)
}

pub(crate) fn declared<'py>(
    declarations: &[ModelMigrationDeclaration<'py>],
    state: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyDict>> {
    let result = PyDict::new(state.py());
    for item in declarations {
        let fingerprint = call_with(
            &native_field(state, "model_fingerprint")?,
            &[
                ("location", item.origin_location.clone()),
                ("model_name", item.origin_model.clone()),
            ],
        )?;
        let explicit = item.origin_model.clone();
        let origin = if explicit.is_truthy()? {
            explicit
        } else if !fingerprint.is_none() {
            native_field(&fingerprint, "node_name")?
        } else {
            state.py().None().into_bound(state.py())
        };
        if !origin.is_none() {
            result.set_item(&item.model_name, origin)?;
        }
    }
    Ok(result)
}

pub(crate) fn renames<'py>(
    runtime: &Bound<'py, PyAny>,
    names: &Bound<'py, PyDict>,
    state: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyDict>> {
    let py = runtime.py();
    let target = native_field(&native_field(runtime, "project")?, "effective_target_name")?;
    let ordered = PyList::empty(py);
    let events: Vec<Bound<'py, PyAny>> = native_field(state, "events")?
        .try_iter()?
        .collect::<PyResult<_>>()?;
    for (index, event) in events.iter().enumerate() {
        ordered.append((
            native_field(event, "created_at")?,
            native_field(event, "event_id")?,
            index,
        ))?;
    }
    ordered.call_method0("sort")?;
    let latest = PyDict::new(py);
    for item in ordered.iter() {
        let index: usize = item.get_item(2)?.extract()?;
        let event = &events[index];
        let event_target = native_field(event, "target_name")?;
        if event_target.is_none() || event_target.eq(&target)? {
            latest.set_item(native_field(event, "destination_model")?, event)?;
        }
    }
    let result = PyDict::new(py);
    for (name, event) in latest.iter() {
        let origin = native_field(&event, "origin_model")?;
        if names.contains(&name)? && !origin.is_none() && !names.contains(&origin)? {
            result.set_item(name, origin)?;
        }
    }
    Ok(result)
}

pub(crate) fn resumed<'py>(
    runtime: &Bound<'py, PyAny>,
    models: &[Bound<'py, PyAny>],
    unbuilt: &Bound<'py, PyDict>,
    state: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyDict>> {
    let result = PyDict::new(runtime.py());
    let target = native_field(&native_field(runtime, "project")?, "effective_target_name")?;
    let events = native_field(state, "events")?;
    for item in models {
        let name = name_object(item)?;
        if !unbuilt.contains(&name)? || !(history(item)? || identity(item)?) {
            continue;
        }
        let destination = sqlbuild_python_project::compiled_views::main::compiled_models::destination::destination(item)?;
        let event = newest(&events, &destination)?;
        if event.is_none() {
            continue;
        }
        let event_target = native_field(&event, "target_name")?;
        if native_field(&event, "destination")?
            .call_method1("matches", (relation(&destination)?,))?
            .is_truthy()?
            && (event_target.is_none() || event_target.eq(&target)?)
            && native_field(&event, "decision")?.eq("renamed")? == identity(item)?
        {
            result.set_item(name, event)?;
        }
    }
    Ok(result)
}

pub(crate) fn key<'py>(location: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyTuple>> {
    let schema = native_field(location, "schema")?;
    let schema = if schema.is_truthy()? {
        schema
    } else {
        pyo3::types::PyString::new(location.py(), "").into_any()
    };
    PyTuple::new(
        location.py(),
        [
            schema.call_method0("lower")?,
            native_field(location, "name")?.call_method0("lower")?,
        ],
    )
}
