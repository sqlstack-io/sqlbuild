//! Audit attachment, target lineage and run scope.

use std::cell::RefCell;
use std::collections::HashMap;

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyModule, PyModuleMethods, PyResult, Python};
use pyo3::types::PyTuple;
use pyo3::{IntoPyObject, pyfunction, wrap_pyfunction};

use sqlbuild_analysis::graph::main::graph_from_indexes::graph_from_indexes;
use sqlbuild_analysis::graph::main::reachable_graph_keys::reachable_graph_keys;
use sqlbuild_analysis::graph::models::{EdgeDirection, GraphIndexes, ProjectGraph};
use sqlbuild_analysis::planner_strategy::constants::{MODEL_REFERENCE_KIND, SOURCE_REFERENCE_KIND};
use sqlbuild_analysis::planner_strategy::main::effective_run_scope::effective_run_scope;
use sqlbuild_analysis::planner_strategy::main::singular_attachment::singular_attachment;
use sqlbuild_analysis::planner_strategy::models::{AuditAttachmentKind, AuditRunScope};

use crate::bindings::_helpers::boundary::python_calls::imported;
use crate::bindings::_helpers::compiled_views::audit_reads;
use crate::bindings::_helpers::graph::keys::native_key;
use crate::bindings::_helpers::planner_scope::values::edges;
use crate::bindings::_helpers::planner_strategy::values::{input_error, optional_string, text};
use crate::bindings::_helpers::records::fields::field as native_field;
use crate::bindings::_helpers::records::ids::ErrorId;

const AUDITING_TYPES_MODULE: &str = "sqlbuild.compiler.auditing.types";
const COMPILE_TYPES_MODULE: &str = "sqlbuild.compiler.compile.types";
const MODEL_RESOURCE_TYPE: &str = "model";
const FINAL_RUN_SCOPE: &str = "final";

/// The names of an audit's references of one kind, in order.
fn reference_names(audit: &Bound<'_, PyAny>, kind: &str) -> PyResult<Vec<String>> {
    let mut names: Vec<String> = Vec::new();
    for reference in audit_reads::references(audit)?.try_iter()? {
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
    let attached_kind = audit_reads::attached_target_kind(audit)?;
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
        let name = audit_reads::attached_target_name(audit)?;
        if name.is_none() {
            return Err(input_error(
                py,
                ErrorId::PlannerInput,
                &format!(
                    "audit '{}': attached audit is missing an attached target name",
                    text(&audit_reads::name(audit)?)?
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
    let py: Python<'_> = audit.py();
    let kind: Bound<'_, PyAny> = audit_reads::attached_target_kind(audit)?;
    let name: Bound<'_, PyAny> = audit_reads::attached_target_name(audit)?;
    if kind.is_none() || name.is_none() {
        return Ok(false);
    }
    let kind: String =
        text(&imported(py, COMPILE_TYPES_MODULE, "AttachedAuditTargetKind")?.call1((kind,))?)?;
    let target = (kind, text(&name)?);
    let lineage = reachable_graph_keys(&[target], graph, EdgeDirection::Upstream);
    for dependency in audit_reads::scope_deps(audit)?.try_iter()? {
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

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(resolve_attachment_kind, module)?)?;
    module.add_function(wrap_pyfunction!(reads_outside_target_lineage, module)?)?;
    module.add_function(wrap_pyfunction!(resolve_effective_run_scope, module)?)
}
