//! Old-name planner object conversions and existing native helper calls.

use sqlbuild_host::entries::models::ExecutorHost;

use crate::planner_migrations::_helpers::column::column_objects::{config, model};
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::native_host::main::host_entry::host_entry;
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use sqlbuild_python_base::records::main::construct::record_class::record_class;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::RecordId;
use sqlbuild_python_front::state_rows::main::migrations::old_name_retention::old_name_retention;
use sqlbuild_python_project::compiled_views::main::compiled_models::destination::destination;

pub(crate) fn relation<'py>(location: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    host_entry(location.py(), ExecutorHost::MigrationRelationForLocation)?.call1((location,))
}

pub(crate) fn location<'py>(
    runtime: &Bound<'py, PyAny>,
    relation: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let mut fields = vec![
        ("database", native_field(relation, "database")?),
        ("schema", native_field(relation, "schema")?),
        ("name", native_field(relation, "name")?),
    ];
    let qualified = call_with(
        &native_field(&native_field(runtime, "adapter")?, "render_qualified_name")?,
        &fields,
    )?;
    fields.push(("qualified_name", qualified));
    new_record_with(runtime.py(), RecordId::CompiledRelationLocation, &fields)
}

pub(crate) fn stores_history(item: &Bound<'_, PyAny>) -> PyResult<bool> {
    let materialization = config(item, "materialized")?;
    Ok(materialization.eq("incremental")? || materialization.eq("snapshot")?)
}

pub(crate) fn retention<'py>(
    item: &Bound<'py, PyAny>,
    project: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    old_name_retention(
        &native_field(
            &sqlbuild_python_project::compiled_views::main::compiled_models::config::config(item)?,
            "values",
        )?,
        project,
    )
}

pub(crate) fn expires<'py>(
    retention: &Bound<'py, PyAny>,
    now: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let duration =
        record_class(now.py(), RecordId::Duration)?.call_method1("parse", (retention,))?;
    if duration.is_none() {
        Ok(now.py().None().into_bound(now.py()))
    } else {
        duration.call_method1("add_to", (now,))
    }
}

pub(crate) fn recorded_view<'py>(
    runtime: &Bound<'py, PyAny>,
    history: &Bound<'py, PyAny>,
    models: &[Bound<'py, PyAny>],
) -> PyResult<Bound<'py, PyAny>> {
    let py = runtime.py();
    let movement = native_field(history, "move")?;
    let created = native_field(history, "created")?;
    let mut reused = py.None().into_bound(py);
    for item in models {
        if relation(&destination(item)?)?
            .call_method1("matches", (native_field(history, "old")?,))?
            .is_truthy()?
        {
            reused = native_field(item, "name")?;
            break;
        }
    }
    model(
        py,
        RecordId::OldNameView,
        &[
            (
                "destination_model",
                native_field(&movement, "destination_model")?,
            ),
            ("old", location(runtime, &native_field(history, "old")?)?),
            ("new", location(runtime, &native_field(history, "new")?)?),
            (
                "expires_at",
                if created.is_none() {
                    py.None().into_bound(py)
                } else {
                    native_field(&created, "expires_at")?
                },
            ),
            (
                "column_aliases",
                if created.is_none() {
                    pyo3::types::PyTuple::empty(py).into_any()
                } else {
                    native_field(&created, "column_aliases")?
                },
            ),
            ("migration_event_id", native_field(&movement, "event_id")?),
            ("target_name", native_field(&movement, "target_name")?),
            ("name_reused_by", reused),
        ],
    )
}
