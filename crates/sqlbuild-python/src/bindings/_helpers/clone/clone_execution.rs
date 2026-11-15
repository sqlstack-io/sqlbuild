//! Execute the prepared clone plan and propagate fingerprints.

use std::collections::HashMap;
use std::time::Instant;

use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyBool, PyDict, PyDictMethods, PyFrozenSet, PyString};
use sqlbuild_core::clone_text::main::clone_relation_flow_text::clone_relation_flow_text;
use sqlbuild_core::clone_text::models::FingerprintProgress;
use sqlbuild_host::entries::models::ExecutorHost;

use crate::bindings::_helpers::clone::clone_command::Invocation;
use crate::bindings::_helpers::clone::clone_progress::{
    CloneFingerprintReporter, CloneItemPrinter,
};
use crate::bindings::_helpers::contracts::spec_resolution::resolve_target_config;
use crate::bindings::_helpers::result_output::event_writer::ExecutionEventWriter;
use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::boundary::main::within::within;
use sqlbuild_python_base::native_host::main::host_entry::host_entry;
use sqlbuild_python_base::records::main::construct::enum_member::enum_member;
use sqlbuild_python_base::records::main::construct::new_record::new_record;
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::main::fields::mapping_items::mapping_items;
use sqlbuild_python_base::records::types::RecordId;

const DESTINATION_ENTRY_GROUPS: [&str; 4] = [
    "destination_source_entries",
    "destination_model_entries",
    "destination_seed_entries",
    "destination_function_entries",
];
const ORIGIN_ENTRY_GROUPS: [&str; 3] = [
    "origin_source_entries",
    "origin_model_entries",
    "origin_seed_entries",
];

/// The pipeline's plan entries of `groups`, in order.
fn entries<'py>(pipeline: &Bound<'py, PyAny>, groups: &[&str]) -> PyResult<Vec<Bound<'py, PyAny>>> {
    let mut found: Vec<Bound<'py, PyAny>> = Vec::new();
    for group in groups {
        for entry in native_field(pipeline, group)?.try_iter()? {
            found.push(entry?);
        }
    }
    Ok(found)
}

/// The number of destination entries, which the header counts.
pub(crate) fn destination_entry_count(pipeline: &Bound<'_, PyAny>) -> PyResult<usize> {
    Ok(entries(pipeline, &DESTINATION_ENTRY_GROUPS)?.len())
}

/// Each destination entry's resource type by name.
pub(crate) fn resource_types_by_name(
    pipeline: &Bound<'_, PyAny>,
) -> PyResult<HashMap<String, String>> {
    entries(pipeline, &DESTINATION_ENTRY_GROUPS)?
        .iter()
        .map(|entry| {
            Ok((
                native_field(entry, "name")?.extract()?,
                native_field(entry, "key")?
                    .getattr("resource_type")?
                    .str()?
                    .extract()?,
            ))
        })
        .collect()
}

fn relation_text(entry: &Bound<'_, PyAny>) -> PyResult<Option<String>> {
    let destination: Bound<'_, PyAny> = native_field(entry, "destination")?;
    let qualified: Bound<'_, PyAny> = native_field(&destination, "qualified_name")?;
    let relation: Bound<'_, PyAny> = if qualified.is_truthy()? {
        qualified
    } else {
        native_field(&destination, "name")?
    };
    if relation.is_none() {
        return Ok(None);
    }
    relation.extract().map(Some)
}

/// The widest plain relation flow among the destination entries.
fn relation_width(pipeline: &Bound<'_, PyAny>) -> PyResult<usize> {
    let mut origins: HashMap<String, Option<String>> = HashMap::new();
    for entry in entries(pipeline, &ORIGIN_ENTRY_GROUPS)? {
        origins.insert(
            native_field(&entry, "name")?.extract()?,
            relation_text(&entry)?,
        );
    }
    let mut width: usize = 0;
    for entry in entries(pipeline, &DESTINATION_ENTRY_GROUPS)? {
        let name: String = native_field(&entry, "name")?.extract()?;
        let destination: Option<String> = relation_text(&entry)?;
        let origin: Option<&str> = origins.get(&name).and_then(Option::as_deref);
        let flow: String = clone_relation_flow_text(&name, (origin, destination.as_deref()));
        width = width.max(flow.chars().count());
    }
    Ok(width)
}

/// Destination relation locations of every planned entry and of each source table.
fn dependency_locations<'py>(
    pipeline: &Bound<'py, PyAny>,
    adapter: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyDict>> {
    let py: Python<'py> = pipeline.py();
    let locations: Bound<'py, PyDict> = PyDict::new(py);
    for entry in entries(pipeline, &DESTINATION_ENTRY_GROUPS)? {
        locations.set_item(
            native_field(&entry, "key")?,
            native_field(&entry, "destination")?,
        )?;
    }
    let plan: Bound<'py, PyAny> = native_field(pipeline, "clone_plan")?;
    let read_map: Bound<'py, PyAny> = native_field(&plan, "source_read_map")?;
    let source_map: Bound<'py, PyAny> = if read_map.is_truthy()? {
        read_map
    } else {
        native_field(&plan, "source_map")?
    };
    let source_type: Bound<'py, PyAny> = enum_member(py, RecordId::CompiledResourceType, "SOURCE")?;
    for pair in mapping_items(&source_map)?.try_iter()? {
        let (name, source): (Bound<'py, PyAny>, Bound<'py, PyAny>) = pair?.extract()?;
        let table: Bound<'py, PyAny> = native_field(&source, "table")?;
        if table.is_none() {
            continue;
        }
        let place = [
            ("database", native_field(&source, "database")?),
            ("schema", native_field(&source, "schema")?),
            ("name", table.clone()),
        ];
        let rendered: Bound<'py, PyAny> =
            call_with(&native_field(adapter, "render_qualified_name")?, &place)?;
        let qualified: Bound<'py, PyAny> = if rendered.is_truthy()? {
            rendered
        } else {
            table
        };
        let [database, schema, table] = place;
        let location: Bound<'py, PyAny> = new_record_with(
            py,
            RecordId::CompiledRelationLocation,
            &[database, schema, table, ("qualified_name", qualified)],
        )?;
        locations.set_item(
            new_record(py, RecordId::CompiledObjectKey, (&source_type, name), None)?,
            location,
        )?;
    }
    Ok(locations)
}

fn target_config<'py>(invocation: &Invocation<'py>) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = invocation.adapter.py();
    resolve_target_config(
        py,
        &native_field(&invocation.discovered, "project_config")?,
        &native_field(&invocation.discovered, "local_config")?,
        &PyString::new(py, &invocation.destination).into_any(),
    )
}

/// The executor input for the whole clone, streaming items through `on_item`.
fn execution_input<'py>(
    invocation: &Invocation<'py>,
    (pipeline, connection): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
    (hard_copy, on_item): (Bound<'py, PyAny>, Bound<'py, PyAny>),
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = pipeline.py();
    let project: Bound<'py, PyAny> = native_field(pipeline, "destination_project")?;
    let plan: Bound<'py, PyAny> = native_field(pipeline, "clone_plan")?;
    let config: Bound<'py, PyAny> = target_config(invocation)?;
    let selected: Vec<Bound<'py, PyAny>> = native_field(pipeline, "destination_model_entries")?
        .try_iter()?
        .map(|entry| native_field(&entry?, "name"))
        .collect::<PyResult<_>>()?;
    let retention: Bound<'py, PyAny> = call_with(
        &host_entry(py, ExecutorHost::BuildDestinationRetentionRequests)?,
        &[
            ("project", project.clone()),
            (
                "adapter_name",
                PyString::new(py, &invocation.adapter_name).into_any(),
            ),
            (
                "selected_model_names",
                PyFrozenSet::new(py, selected)?.into_any(),
            ),
            (
                "namespace_owned",
                native_field(&config, "owns_time_travel_retention_namespace")?,
            ),
        ],
    )?;
    let allow_decrease: bool = native_field(&config, "time_travel_retention_decrease")?
        .eq(enum_member(py, RecordId::RetentionDecreasePolicy, "ALLOW")?)?;
    let source_entries: Bound<'py, PyAny> = call_with(
        &host_entry(py, ExecutorHost::CloneSourceEntries)?,
        &[
            ("origin", native_field(pipeline, "origin_source_entries")?),
            (
                "destination",
                native_field(pipeline, "destination_source_entries")?,
            ),
        ],
    )?;
    let transient: Bound<'py, PyAny> = call_with(
        &host_entry(py, ExecutorHost::BuildDestinationTransientModels)?,
        &[("project", project.clone())],
    )?;
    let mut fields: Vec<(&str, Bound<'py, PyAny>)> = vec![("source_entries", source_entries)];
    for group in [
        "origin_model_entries",
        "destination_model_entries",
        "origin_seed_entries",
        "destination_seed_entries",
        "destination_function_entries",
    ] {
        fields.push((group, native_field(pipeline, group)?));
    }
    fields.extend([
        ("execution_order", native_field(&plan, "execution_order")?),
        ("adapter", invocation.adapter.clone()),
        ("destination_connection", connection.clone()),
        ("hard_copy", hard_copy),
        ("run_id", native_field(&project, "run_id")?),
        (
            "query_change_tracking",
            native_field(&project, "settings")?.getattr("query_change_tracking")?,
        ),
        ("upstream_deps", native_field(&plan, "upstream_deps")?),
        (
            "dependency_locations",
            dependency_locations(pipeline, &invocation.adapter)?.into_any(),
        ),
        ("destination_transient_models", transient),
        ("destination_retention_requests", retention),
        (
            "allow_namespace_retention_decrease",
            PyBool::new(py, allow_decrease).to_owned().into_any(),
        ),
        ("on_item", on_item),
    ]);
    call_with(&host_entry(py, ExecutorHost::CloneExecutionInput)?, &fields)
}

/// Copy fingerprints of the cloned relations, reporting what was not confirmed on failure.
fn copy_fingerprints<'py>(
    invocation: &Invocation<'py>,
    (pipeline, connection): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
    result: &Bound<'py, PyAny>,
) -> PyResult<()> {
    let py: Python<'py> = pipeline.py();
    let reporter: Bound<'py, CloneFingerprintReporter> = Bound::new(
        py,
        CloneFingerprintReporter {
            stream: invocation.stream.clone().unbind(),
            style: invocation.style,
            progress: FingerprintProgress::default(),
        },
    )?;
    let project: Bound<'py, PyAny> = native_field(pipeline, "destination_project")?;
    let mut fields: Vec<(&str, Bound<'py, PyAny>)> = vec![("result", result.clone())];
    for group in [
        "origin_model_entries",
        "destination_model_entries",
        "origin_seed_entries",
        "destination_seed_entries",
    ] {
        fields.push((group, native_field(pipeline, group)?));
    }
    fields.extend([
        ("adapter", invocation.adapter.clone()),
        ("destination_connection", connection.clone()),
        ("run_id", native_field(&project, "run_id")?),
        (
            "query_change_tracking",
            native_field(&project, "settings")?.getattr("query_change_tracking")?,
        ),
        ("on_progress", reporter.clone().into_any()),
    ]);
    let lifecycle: Bound<'py, PyAny> = call_with(
        &host_entry(py, ExecutorHost::OperationLifecycle)?,
        &[
            ("operation_kind", PyString::new(py, "clone").into_any()),
            (
                "operation_name",
                PyString::new(py, "clone_finalization").into_any(),
            ),
        ],
    )?;
    let copied: PyResult<Bound<'py, PyAny>> = within(&lifecycle, |_| {
        call_with(
            &host_entry(py, ExecutorHost::CopyCloneFingerprints)?,
            &fields,
        )
    });
    if let Err(error) = copied {
        reporter.borrow().write_interrupted(py)?;
        return Err(error);
    }
    Ok(())
}

/// Execute the clone, streaming items, then copy fingerprints; returns the result and elapsed seconds.
pub(crate) fn execute_clone_plan<'py>(
    request: &Bound<'py, PyAny>,
    invocation: &Invocation<'py>,
    (pipeline, connection): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
) -> PyResult<(Bound<'py, PyAny>, f64)> {
    let py: Python<'py> = pipeline.py();
    let started: Instant = Instant::now();
    let event_writer: Bound<'py, PyAny> = ExecutionEventWriter::new(
        py,
        Some(native_field(request, "event_output_path")?).filter(|value| !value.is_none()),
    )
    .and_then(|value| pyo3::Bound::new(py, value))
    .map(pyo3::Bound::into_any)?;
    let printer: Py<CloneItemPrinter> = Py::new(
        py,
        CloneItemPrinter {
            stream: invocation.stream.clone().unbind(),
            style: invocation.style,
            relation_width: relation_width(pipeline)?,
            event_writer: event_writer.clone().unbind(),
            resource_types: resource_types_by_name(pipeline)?,
        },
    )?;
    let executed: PyResult<Bound<'py, PyAny>> = (|| {
        let input: Bound<'py, PyAny> = execution_input(
            invocation,
            (pipeline, connection),
            (
                native_field(request, "hard_copy")?,
                printer.into_bound(py).into_any(),
            ),
        )?;
        call_with(
            &host_entry(py, ExecutorHost::ExecuteClone)?,
            &[("inputs", input)],
        )
    })();
    let closed: PyResult<Bound<'py, PyAny>> = event_writer.call_method0("close");
    let result: Bound<'py, PyAny> = executed?;
    closed?;
    copy_fingerprints(invocation, (pipeline, connection), &result)?;
    Ok((result, started.elapsed().as_secs_f64()))
}
