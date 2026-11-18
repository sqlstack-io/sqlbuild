//! Audit attachment, target lineage and run scope.

use crate::planner_strategy::main::values::input_error::input_error;
use crate::planner_strategy::main::values::text::text;
use std::cell::RefCell;
use std::collections::HashMap;
use std::collections::HashSet;

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::PyTuple;
use pyo3::{IntoPyObject, pyfunction};

use sqlbuild_analysis::graph::main::graph_from_indexes::graph_from_indexes;
use sqlbuild_analysis::graph::main::reachable_graph_keys::reachable_graph_keys;
use sqlbuild_analysis::graph::models::{EdgeDirection, GraphIndexes, ProjectGraph};
use sqlbuild_analysis::planner_strategy::constants::{MODEL_REFERENCE_KIND, SOURCE_REFERENCE_KIND};
use sqlbuild_analysis::planner_strategy::main::effective_run_scope::effective_run_scope;
use sqlbuild_analysis::planner_strategy::main::singular_attachment::singular_attachment;
use sqlbuild_analysis::planner_strategy::models::{AuditAttachmentKind, AuditRunScope};

use crate::planner_strategy::_helpers::values::optional_string;
use sqlbuild_python_base::boundary::main::imported::imported;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::ErrorId;
use sqlbuild_python_project::graph::main::nodes::native_key::native_key;
use sqlbuild_python_project::planner_scope::main::edges::edges;
use sqlbuild_python_project::resource_views::main::audit_reads::attached_target_kind::attached_target_kind;
use sqlbuild_python_project::resource_views::main::audit_reads::attached_target_name::attached_target_name;
use sqlbuild_python_project::resource_views::main::audit_reads::references::references;
use sqlbuild_python_project::resource_views::main::audit_reads::scope_deps::scope_deps;

const AUDITING_TYPES_MODULE: &str = "sqlbuild.compiler.auditing.types";
const COMPILE_TYPES_MODULE: &str = "sqlbuild.compiler.compile.types";
const MODEL_RESOURCE_TYPE: &str = "model";
const FINAL_RUN_SCOPE: &str = "final";

/// The names of an audit's references of one kind, in order.
fn reference_names(audit: &Bound<'_, PyAny>, kind: &str) -> PyResult<Vec<String>> {
    let mut names: Vec<String> = Vec::new();
    for reference in references(audit)?.try_iter()? {
        let reference: Bound<'_, PyAny> = reference?;
        if text(&native_field(&reference, "ref_kind")?)? == kind {
            names.push(text(&native_field(&reference, "ref_name")?)?);
        }
    }
    Ok(names)
}

fn attachment_value(kind: AuditAttachmentKind) -> &'static str {
    match kind {
        AuditAttachmentKind::Source => "source",
        AuditAttachmentKind::Seed => "seed",
        AuditAttachmentKind::Model => "model",
        AuditAttachmentKind::End => "end",
    }
}

/// Audit attachment kind and attached target name; attached audits gate their target.
#[pyfunction]
#[pyo3(signature = (*, audit, downstream_deps))]
pub(crate) fn resolve_attachment_kind<'py>(
    audit: &Bound<'py, PyAny>,
    downstream_deps: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyTuple>> {
    let py: Python<'py> = audit.py();
    let graph = graph_from_indexes(GraphIndexes {
        downstream: edges(downstream_deps)?,
        ..GraphIndexes::default()
    });
    let (kind, name) = attachment(audit, &graph)?;
    PyTuple::new(
        py,
        [
            imported(py, AUDITING_TYPES_MODULE, "AuditAttachmentKind")?.call1((kind,))?,
            name.into_pyobject(py)?.into_any(),
        ],
    )
}

/// Resolve an attachment against the existing native graph, without public key or tuple objects.
pub(crate) fn attachment(
    audit: &Bound<'_, PyAny>,
    graph: &ProjectGraph,
) -> PyResult<(String, Option<String>)> {
    let py = audit.py();
    let attached_kind = attached_target_kind(audit)?;
    let (kind, name): (String, Option<String>) = if attached_kind.is_none() {
        let reached = RefCell::new(HashMap::new());
        let reaches = |from: &str, to: &str| -> PyResult<bool> {
            Ok(reached
                .borrow_mut()
                .entry(from.to_owned())
                .or_insert_with(|| {
                    reachable_graph_keys(
                        &[(MODEL_RESOURCE_TYPE.to_owned(), from.to_owned())],
                        graph,
                        EdgeDirection::Downstream,
                    )
                })
                .contains(&(MODEL_RESOURCE_TYPE.to_owned(), to.to_owned())))
        };
        let (kind, name) = singular_attachment(
            &reference_names(audit, MODEL_REFERENCE_KIND)?,
            &reference_names(audit, SOURCE_REFERENCE_KIND)?,
            &reaches,
        )?;
        (attachment_value(kind).to_owned(), name)
    } else {
        let name = attached_target_name(audit)?;
        if name.is_none() {
            return Err(input_error(
                py,
                ErrorId::PlannerInput,
                &format!(
                    "audit '{}': attached audit is missing an attached target name",
                    text(
                        &sqlbuild_python_project::resource_views::main::audit_reads::name::name(
                            audit
                        )?
                    )?
                ),
                None,
            ));
        }
        let kind: String = text(
            &imported(py, COMPILE_TYPES_MODULE, "AttachedAuditTargetKind")?
                .call1((attached_kind,))?,
        )?;
        (kind, Some(text(&name)?))
    };
    Ok((kind, name))
}

/// Whether an attached audit reads anything besides its target and the target's upstream.
#[pyfunction]
#[pyo3(signature = (*, audit, upstream_deps))]
pub(crate) fn reads_outside_target_lineage(
    audit: &Bound<'_, PyAny>,
    upstream_deps: &Bound<'_, PyAny>,
) -> PyResult<bool> {
    let graph = graph_from_indexes(GraphIndexes {
        upstream: edges(upstream_deps)?,
        ..GraphIndexes::default()
    });
    outside_target_lineage(audit, &graph)
}

/// Check target lineage using native keys and the project's edge indexes.
pub(crate) fn outside_target_lineage(
    audit: &Bound<'_, PyAny>,
    graph: &ProjectGraph,
) -> PyResult<bool> {
    let Some(target) = lineage_target(audit)? else {
        return Ok(false);
    };
    let lineage = reachable_graph_keys(&[target], graph, EdgeDirection::Upstream);
    outside_lineage(audit, &lineage)
}

/// `outside_target_lineage` of each audit, walking each distinct target's lineage once.
///
/// Many audits attach to one model; walking its upstream lineage per audit dominated planning.
/// One target's lineage is held at a time.
pub(crate) fn outside_target_lineages(
    audits: &[Bound<'_, PyAny>],
    graph: &ProjectGraph,
) -> PyResult<Vec<bool>> {
    let mut outside: Vec<bool> = vec![false; audits.len()];
    let mut by_target: HashMap<(String, String), Vec<usize>> = HashMap::new();
    for (position, audit) in audits.iter().enumerate() {
        if let Some(target) = lineage_target(audit)? {
            by_target.entry(target).or_default().push(position);
        }
    }
    for (target, positions) in by_target {
        let lineage = reachable_graph_keys(&[target], graph, EdgeDirection::Upstream);
        for position in positions {
            outside[position] = outside_lineage(&audits[position], &lineage)?;
        }
    }
    Ok(outside)
}

/// The key an attached audit's target lineage starts from, or `None` for an unattached audit.
fn lineage_target(audit: &Bound<'_, PyAny>) -> PyResult<Option<(String, String)>> {
    let py: Python<'_> = audit.py();
    let kind: Bound<'_, PyAny> = attached_target_kind(audit)?;
    let name: Bound<'_, PyAny> = attached_target_name(audit)?;
    if kind.is_none() || name.is_none() {
        return Ok(None);
    }
    let kind: String =
        text(&imported(py, COMPILE_TYPES_MODULE, "AttachedAuditTargetKind")?.call1((kind,))?)?;
    Ok(Some((kind, text(&name)?)))
}

/// Whether the audit reads a dependency outside `lineage`.
fn outside_lineage(
    audit: &Bound<'_, PyAny>,
    lineage: &HashSet<(String, String)>,
) -> PyResult<bool> {
    for dependency in scope_deps(audit)?.try_iter()? {
        if !lineage.contains(&native_key(&dependency?)?) {
            return Ok(true);
        }
    }
    Ok(false)
}

/// The run scope after degradation: delta-and-final only for incremental and snapshot targets.
#[pyfunction]
#[pyo3(signature = (*, requested_run_scope, attached_model_materialization))]
pub(crate) fn resolve_effective_run_scope<'py>(
    requested_run_scope: &Bound<'py, PyAny>,
    attached_model_materialization: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = requested_run_scope.py();
    let requested: AuditRunScope = if text(requested_run_scope)? == FINAL_RUN_SCOPE {
        AuditRunScope::Final
    } else {
        AuditRunScope::DeltaAndFinal
    };
    let materialization: Option<String> = optional_string(attached_model_materialization)?;
    let scope: &str = match effective_run_scope(requested, materialization.as_deref()) {
        AuditRunScope::Final => FINAL_RUN_SCOPE,
        AuditRunScope::DeltaAndFinal => "delta_and_final",
    };
    imported(py, AUDITING_TYPES_MODULE, "AuditRunScope")?.call1((scope,))
}
