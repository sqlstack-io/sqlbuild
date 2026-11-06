//! Assemble audit plan dataclasses using native rendering and scheduling.

use sqlbuild_analysis::assembly::project::types::ObjectKey;
use sqlbuild_analysis::graph::main::graph_from_indexes::graph_from_indexes;
use sqlbuild_analysis::graph::models::GraphIndexes;
use std::collections::HashSet;

use crate::bindings::_helpers::attachments::reference_markers::assert_no_unresolved_sql_markers;
use crate::bindings::_helpers::auditing::render::render_audit_sql;
use crate::bindings::_helpers::boundary::python_calls::keywords;
use crate::bindings::_helpers::compiled_views::audit_reads;
use crate::bindings::_helpers::compiled_views::project_reads;
use crate::bindings::_helpers::graph::keys::native_key;
use crate::bindings::_helpers::graph::project_graph::NativeProjectGraph;
use crate::bindings::_helpers::planner_phases::objects::object;
use crate::bindings::_helpers::planner_scope::retained::scope_data;
use crate::bindings::_helpers::planner_scope::values::edges;
use crate::bindings::_helpers::planner_strategy::audits::{
    attachment, outside_target_lineage, resolve_effective_run_scope,
};
use crate::bindings::_helpers::records::construct::new_record;
use pyo3::IntoPyObject;
use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyResult};
use pyo3::pyfunction;
use pyo3::types::{PyBool, PyDict, PyDictMethods, PyList, PyListMethods, PyString};

use crate::bindings::_helpers::boundary::keyword_request::Request;
use crate::bindings::_helpers::boundary::python_calls::imported;

use crate::bindings::_helpers::planner_strategy::constants::{
    AUDIT_TYPES_MODULE, COMPILE_TYPES_MODULE, MODEL_ATTACHMENT, SOURCE_ATTACHMENT,
};
use crate::bindings::_helpers::planner_strategy::models::AuditPlanning;
use crate::bindings::_helpers::records::fields::field as native_field;
use crate::bindings::_helpers::records::fields::mapping_get;
use crate::bindings::_helpers::records::ids::RecordId;

/// One attribute read of a compiled audit.
type AuditRead<'py> = fn(&Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>>;

/// Render and validate one audit query with the supplied adapter callbacks.
fn render_query<'py>(
    request: &AuditPlanning<'py>,
    sql: Bound<'py, PyAny>,
    context: &str,
) -> PyResult<Bound<'py, PyAny>> {
    let py = sql.py();
    let resolved = object(
        py,
        render_audit_sql(Some(&keywords(
            py,
            &[
                ("unresolved_sql", sql),
                ("model_locations", request.model_locations.clone()),
                ("seed_locations", request.seed_locations.clone()),
                ("source_map", request.source_map.clone()),
                ("adapter", request.adapter.clone()),
            ],
        )?))?,
    )?;
    object(
        py,
        assert_no_unresolved_sql_markers(py, resolved.downcast::<PyString>()?, context)?,
    )?;
    Ok(resolved)
}

/// Resolve an audit enum, applying the default only to absent values.
fn audit_enum<'py>(
    value: Bound<'py, PyAny>,
    enum_name: &str,
    default: &str,
) -> PyResult<Bound<'py, PyAny>> {
    let py = value.py();
    imported(py, AUDIT_TYPES_MODULE, enum_name)?.call1((if value.is_none() {
        PyString::new(py, default).into_any()
    } else {
        value
    },))
}

/// Build an audit entry while retaining the public Python plan dataclass.
#[pyfunction(name = "plan_audit")]
#[pyo3(signature = (**request))]
pub(crate) fn plan_audit_of<'py>(
    request: Option<&Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    let request = Request::new("plan_audit", request)?;
    plan_audit(
        &request.get("audit")?,
        &AuditPlanning {
            model_locations: request.get("model_locations")?,
            seed_locations: request.get("seed_locations")?,
            source_map: request.get("source_map")?,
            adapter: request.get("adapter")?,
            model_materializations: request.get("model_materializations")?,
            lineage: Py::new(
                request.py(),
                NativeProjectGraph::new(graph_from_indexes(GraphIndexes {
                    upstream: edges(&request.get("lineage_upstream_deps")?)?,
                    downstream: edges(&request.get("lineage_downstream_deps")?)?,
                    ..GraphIndexes::default()
                })),
            )?,
        },
    )
}

/// One audit's plan entry, rendered against the planning relations.
fn plan_audit<'py>(
    audit: &Bound<'py, PyAny>,
    request: &AuditPlanning<'py>,
) -> PyResult<Bound<'py, PyAny>> {
    let py = audit.py();
    let audit = audit.clone();
    let name: String = audit_reads::name(&audit)?.extract()?;
    let resolved = render_query(
        request,
        audit_reads::sql_body(&audit)?,
        &format!("audit '{name}' planned SQL"),
    )?;
    let evidence = audit_reads::evidence_sql(&audit)?;
    let evidence_resolved = if evidence.is_none() {
        py.None().into_bound(py)
    } else {
        render_query(
            request,
            evidence,
            &format!("audit '{name}' planned evidence SQL"),
        )?
    };
    let lineage = request.lineage.bind(py).borrow();
    let (kind_text, target_name) = attachment(&audit, lineage.graph())?;
    let kind = audit_enum(
        PyString::new(py, &kind_text).into_any(),
        "AuditAttachmentKind",
        "end",
    )?;
    let target = target_name.as_deref().into_pyobject(py)?.into_any();
    let materialization = if kind_text == MODEL_ATTACHMENT && target_name.is_some() {
        mapping_get(&request.model_materializations, &target)?
    } else {
        py.None().into_bound(py)
    };
    let requested = audit_enum(audit_reads::run_scope(&audit)?, "AuditRunScope", "final")?;
    let severity = audit_enum(audit_reads::severity(&audit)?, "AuditSeverity", "warn")?;
    let effective = resolve_effective_run_scope(&requested, &materialization)?;
    let mut target_kind = audit_reads::attached_target_kind(&audit)?;
    if target_kind.is_none() && (kind_text == MODEL_ATTACHMENT || kind_text == SOURCE_ATTACHMENT) {
        target_kind =
            imported(py, COMPILE_TYPES_MODULE, "AttachedAuditTargetKind")?.call1((kind_text,))?;
    }
    let values = PyDict::new(py);
    let reads: [(&str, AuditRead<'py>); 12] = [
        ("key", audit_reads::key),
        ("name", audit_reads::name),
        ("definition_name", audit_reads::definition_name),
        ("evaluation_mode", audit_reads::evaluation_mode),
        ("thresholds", audit_reads::thresholds),
        ("minimum_samples", audit_reads::minimum_samples),
        ("evidence_limit", audit_reads::evidence_limit),
        ("scope_deps", audit_reads::scope_deps),
        ("attached_column_name", audit_reads::attached_column_name),
        ("always_run", audit_reads::always_run),
        ("description", audit_reads::description),
        ("meta", audit_reads::meta),
    ];
    for (field, read) in reads {
        values.set_item(field, read(&audit)?)?;
    }
    let measurement = audit_reads::measurement_contract(&audit)?;
    for field in ["value_column", "sample_count_column", "sample_unit"] {
        values.set_item(
            field,
            if measurement.is_none() {
                py.None().into_bound(py)
            } else {
                native_field(&measurement, field)?
            },
        )?;
    }
    for (field, value) in [
        ("resolved_sql", resolved),
        ("unresolved_sql", audit_reads::sql_body(&audit)?),
        ("attachment_kind", kind),
        ("severity", severity),
        ("requested_run_scope", requested),
        ("effective_run_scope", effective),
        ("evidence_resolved_sql", evidence_resolved),
        (
            "evidence_unresolved_sql",
            audit_reads::evidence_sql(&audit)?,
        ),
        ("attached_target_kind", target_kind),
        ("attached_target_name", target),
        (
            "reads_outside_target_lineage",
            PyBool::new(py, outside_target_lineage(&audit, lineage.graph())?)
                .to_owned()
                .into_any(),
        ),
    ] {
        values.set_item(field, value)?;
    }
    new_record(py, RecordId::AuditPlanEntry, (), Some(&values))
}

/// Select attached audits by their logical target and singular audits by dependency overlap.
pub(crate) fn audit_is_selected(
    audit: &Bound<'_, PyAny>,
    selected_keys: &HashSet<ObjectKey>,
) -> PyResult<bool> {
    let py = audit.py();
    let kind = audit_reads::attached_target_kind(audit)?;
    let name = audit_reads::attached_target_name(audit)?;
    if !kind.is_none() && !name.is_none() {
        let target_value = native_field(
            &imported(py, COMPILE_TYPES_MODULE, "AttachedAuditTargetKind")?.call1((kind,))?,
            "value",
        )?;
        let key = (target_value.extract()?, name.extract()?);
        return Ok(selected_keys.contains(&key));
    }
    for dependency in audit_reads::scope_deps(audit)?.try_iter()? {
        if selected_keys.contains(&native_key(&dependency?)?) {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Assemble selected audits in compiled-project order without querying lineage for an empty selection.
pub(crate) fn build_selected_audit_entries<'py>(
    request: Option<&Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyList>> {
    let request = Request::new("build_selected_audit_entries", request)?;
    let py = request.py();
    let project = request.get("project")?;
    let scope = scope_data(&request.get("scope")?)?;
    let mut selected: Vec<Bound<'py, PyAny>> = Vec::new();
    for audit in project_reads::audits(&project)?.try_iter()? {
        let audit = audit?;
        if audit_is_selected(&audit, &scope.selected)? {
            selected.push(audit);
        }
    }
    let entries = PyList::empty(py);
    if selected.is_empty() {
        return Ok(entries);
    }
    let relations = request.get("relations")?;
    let planning = AuditPlanning {
        model_locations: native_field(&relations, "model_locations")?,
        seed_locations: native_field(&relations, "seed_locations")?,
        source_map: native_field(&relations, "source_read_map")?,
        adapter: request.get("adapter")?,
        model_materializations: request.get("model_materializations")?,
        lineage: native_field(&project, "lineage_graph")?.extract()?,
    };
    for audit in selected {
        entries.append(plan_audit(&audit, &planning)?)?;
    }
    Ok(entries)
}
