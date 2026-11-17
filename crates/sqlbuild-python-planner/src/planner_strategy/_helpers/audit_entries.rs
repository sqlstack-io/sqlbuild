//! Assemble audit plan dataclasses using native rendering and scheduling.

use sqlbuild_analysis::graph::main::graph_from_indexes::graph_from_indexes;
use sqlbuild_analysis::graph::models::GraphIndexes;
use sqlbuild_python_base::native_host::models::Dialect;

use crate::planner_strategy::_helpers::audits::{
    attachment, outside_target_lineage, resolve_effective_run_scope,
};
use pyo3::IntoPyObject;
use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyResult};
use pyo3::pyfunction;
use pyo3::types::{PyBool, PyDict, PyDictMethods, PyString};
use sqlbuild_python_base::boundary::main::keywords::keywords;
use sqlbuild_python_base::records::main::construct::new_record::new_record;
use sqlbuild_python_front::attachments::main::assert_no_unresolved_sql_markers::assert_no_unresolved_sql_markers;
use sqlbuild_python_project::auditing::main::render_audit_sql::render_audit_sql;
use sqlbuild_python_project::graph::models::NativeProjectGraph;
use sqlbuild_python_project::planner_phases::main::object::object;
use sqlbuild_python_project::planner_scope::main::edges::edges;
use sqlbuild_python_project::resource_views::main::audit_reads::always_run::always_run;
use sqlbuild_python_project::resource_views::main::audit_reads::attached_column_name::attached_column_name;
use sqlbuild_python_project::resource_views::main::audit_reads::attached_target_kind::attached_target_kind;
use sqlbuild_python_project::resource_views::main::audit_reads::definition_name::definition_name;
use sqlbuild_python_project::resource_views::main::audit_reads::description::description;
use sqlbuild_python_project::resource_views::main::audit_reads::evaluation_mode::evaluation_mode;
use sqlbuild_python_project::resource_views::main::audit_reads::evidence_limit::evidence_limit;
use sqlbuild_python_project::resource_views::main::audit_reads::evidence_sql::evidence_sql;
use sqlbuild_python_project::resource_views::main::audit_reads::measurement_contract::measurement_contract;
use sqlbuild_python_project::resource_views::main::audit_reads::meta::meta;
use sqlbuild_python_project::resource_views::main::audit_reads::minimum_samples::minimum_samples;
use sqlbuild_python_project::resource_views::main::audit_reads::run_scope::run_scope;
use sqlbuild_python_project::resource_views::main::audit_reads::scope_deps::scope_deps;
use sqlbuild_python_project::resource_views::main::audit_reads::sql_body::sql_body;
use sqlbuild_python_project::resource_views::main::audit_reads::thresholds::thresholds;

use sqlbuild_python_base::boundary::main::imported::imported;
use sqlbuild_python_base::boundary::models::Request;

use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::main::fields::mapping_get::mapping_get;
use sqlbuild_python_base::records::types::RecordId;
use sqlbuild_python_project::planner_strategy::constants::AUDIT_TYPES_MODULE;
use sqlbuild_python_project::planner_strategy::constants::COMPILE_TYPES_MODULE;
use sqlbuild_python_project::planner_strategy::constants::MODEL_ATTACHMENT;
use sqlbuild_python_project::planner_strategy::constants::SOURCE_ATTACHMENT;
use sqlbuild_python_project::planner_strategy::models::AuditPlanning;

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
                ("adapter", request.dialect.clone().into_any()),
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
            dialect: Dialect::passed(request.get("dialect")?)?,
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
pub(crate) fn plan_audit<'py>(
    audit: &Bound<'py, PyAny>,
    request: &AuditPlanning<'py>,
) -> PyResult<Bound<'py, PyAny>> {
    let py = audit.py();
    let audit = audit.clone();
    let name: String =
        sqlbuild_python_project::resource_views::main::audit_reads::name::name(&audit)?
            .extract()?;
    let resolved = render_query(
        request,
        sql_body(&audit)?,
        &format!("audit '{name}' planned SQL"),
    )?;
    let evidence = evidence_sql(&audit)?;
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
    let requested = audit_enum(run_scope(&audit)?, "AuditRunScope", "final")?;
    let severity = audit_enum(
        sqlbuild_python_project::resource_views::main::audit_reads::severity::severity(&audit)?,
        "AuditSeverity",
        "warn",
    )?;
    let effective = resolve_effective_run_scope(&requested, &materialization)?;
    let mut target_kind = attached_target_kind(&audit)?;
    if target_kind.is_none() && (kind_text == MODEL_ATTACHMENT || kind_text == SOURCE_ATTACHMENT) {
        target_kind =
            imported(py, COMPILE_TYPES_MODULE, "AttachedAuditTargetKind")?.call1((kind_text,))?;
    }
    let values = PyDict::new(py);
    let reads: [(&str, AuditRead<'py>); 12] = [
        (
            "key",
            sqlbuild_python_project::resource_views::main::audit_reads::key::key,
        ),
        (
            "name",
            sqlbuild_python_project::resource_views::main::audit_reads::name::name,
        ),
        ("definition_name", definition_name),
        ("evaluation_mode", evaluation_mode),
        ("thresholds", thresholds),
        ("minimum_samples", minimum_samples),
        ("evidence_limit", evidence_limit),
        ("scope_deps", scope_deps),
        ("attached_column_name", attached_column_name),
        ("always_run", always_run),
        ("description", description),
        ("meta", meta),
    ];
    for (field, read) in reads {
        values.set_item(field, read(&audit)?)?;
    }
    let measurement = measurement_contract(&audit)?;
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
        ("unresolved_sql", sql_body(&audit)?),
        ("attachment_kind", kind),
        ("severity", severity),
        ("requested_run_scope", requested),
        ("effective_run_scope", effective),
        ("evidence_resolved_sql", evidence_resolved),
        ("evidence_unresolved_sql", evidence_sql(&audit)?),
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
