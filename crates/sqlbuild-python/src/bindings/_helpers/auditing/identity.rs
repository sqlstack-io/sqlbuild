//! Audit gate identities of planned audits, built as `AuditGateIdentity` objects.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyModule, PyModuleMethods, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods, PyTuple};
use pyo3::{IntoPyObject, pyfunction, wrap_pyfunction};
use sqlbuild_analysis::auditing::main::audit_gate_identity::audit_gate_identity;
use sqlbuild_analysis::auditing::models::{
    GateIdentity, MeasurementFacts, PlannedAudit, ThresholdBound, Thresholds,
};
use sqlbuild_python_base::records::main::construct::record_class::record_class;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::RecordId;

const MEASUREMENT_MODE: &str = "measurement";

fn enum_value(value: &Bound<'_, PyAny>) -> PyResult<String> {
    native_field(value, "value")?.extract()
}

/// One `MeasurementThresholdBound`, or `None`.
pub(crate) fn threshold_bound(bound: &Bound<'_, PyAny>) -> PyResult<Option<ThresholdBound>> {
    if bound.is_none() {
        return Ok(None);
    }
    let operator: String = enum_value(&native_field(bound, "operator")?)?;
    Ok(Some(match operator.as_str() {
        "below" => ThresholdBound::Below(native_field(bound, "limit")?.extract()?),
        "above" => ThresholdBound::Above(native_field(bound, "limit")?.extract()?),
        _ => ThresholdBound::Outside(
            native_field(bound, "lower")?.extract()?,
            native_field(bound, "upper")?.extract()?,
        ),
    }))
}

/// One `MeasurementThresholds`, or `None`.
pub(crate) fn thresholds(value: &Bound<'_, PyAny>) -> PyResult<Option<Thresholds>> {
    if value.is_none() {
        return Ok(None);
    }
    Ok(Some(Thresholds {
        warn: threshold_bound(&native_field(value, "warn")?)?,
        error: threshold_bound(&native_field(value, "error")?)?,
    }))
}

fn planned_audit(audit: &Bound<'_, PyAny>) -> PyResult<PlannedAudit> {
    let measurement: Option<MeasurementFacts> =
        if enum_value(&native_field(audit, "evaluation_mode")?)? == MEASUREMENT_MODE {
            Some(MeasurementFacts {
                value_column: native_field(audit, "value_column")?.extract()?,
                sample_count_column: native_field(audit, "sample_count_column")?.extract()?,
                sample_unit: native_field(audit, "sample_unit")?.extract()?,
                thresholds: thresholds(&native_field(audit, "thresholds")?)?,
                minimum_samples: native_field(audit, "minimum_samples")?.extract()?,
                evidence_unresolved_sql: native_field(audit, "evidence_unresolved_sql")?
                    .extract()?,
                evidence_resolved_sql: native_field(audit, "evidence_resolved_sql")?.extract()?,
            })
        } else {
            None
        };
    Ok(PlannedAudit {
        name: native_field(audit, "name")?.extract()?,
        attachment_kind: enum_value(&native_field(audit, "attachment_kind")?)?,
        attached_target_name: native_field(audit, "attached_target_name")?.extract()?,
        attached_column_name: native_field(audit, "attached_column_name")?.extract()?,
        severity: enum_value(&native_field(audit, "severity")?)?,
        run_scope: enum_value(&native_field(audit, "effective_run_scope")?)?,
        always_run: native_field(audit, "always_run")?.is_truthy()?,
        unresolved_sql: native_field(audit, "unresolved_sql")?.extract()?,
        resolved_sql: native_field(audit, "resolved_sql")?.extract()?,
        measurement,
    })
}

fn keyword_call<'py>(
    callable: &Bound<'py, PyAny>,
    pairs: Vec<(&str, Bound<'py, PyAny>)>,
) -> PyResult<Bound<'py, PyAny>> {
    let keywords: Bound<'py, PyDict> = PyDict::new(callable.py());
    for (name, value) in pairs {
        keywords.set_item(name, value)?;
    }
    callable.call((), Some(&keywords))
}

/// Target-neutral and target-specific identity hashes for planned audits.
#[pyfunction]
#[pyo3(signature = (*, audits))]
fn build_audit_gate_identity<'py>(
    py: Python<'py>,
    audits: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let entries: Vec<Bound<'py, PyAny>> = audits.try_iter()?.collect::<PyResult<_>>()?;
    let planned: Vec<PlannedAudit> = entries.iter().map(planned_audit).collect::<PyResult<_>>()?;
    let gate: GateIdentity = audit_gate_identity(&planned);
    let identity_type: Bound<'py, PyAny> = record_class(py, RecordId::AuditIdentity)?;
    let mut identities: Vec<Bound<'py, PyAny>> = Vec::with_capacity(gate.audits.len());
    for identity in gate.audits {
        let entry: &Bound<'py, PyAny> = &entries[identity.index];
        identities.push(keyword_call(
            &identity_type,
            vec![
                (
                    "binding_key",
                    identity.binding_key.into_pyobject(py)?.into_any(),
                ),
                ("audit_name", native_field(entry, "name")?),
                (
                    "definition_fingerprint",
                    identity
                        .definition_fingerprint
                        .into_pyobject(py)?
                        .into_any(),
                ),
                (
                    "execution_fingerprint",
                    identity.execution_fingerprint.into_pyobject(py)?.into_any(),
                ),
                ("severity", native_field(entry, "severity")?),
                (
                    "run_scope_phase",
                    native_field(entry, "effective_run_scope")?.getattr("value")?,
                ),
                (
                    "attachment_kind",
                    native_field(entry, "attachment_kind")?.getattr("value")?,
                ),
                (
                    "attached_target_name",
                    native_field(entry, "attached_target_name")?,
                ),
                (
                    "attached_column_name",
                    native_field(entry, "attached_column_name")?,
                ),
                ("always_run", native_field(entry, "always_run")?),
            ],
        )?);
    }
    keyword_call(
        &record_class(py, RecordId::AuditGateIdentity)?,
        vec![
            (
                "binding_set_hash",
                gate.binding_set_hash.into_pyobject(py)?.into_any(),
            ),
            (
                "blocking_set_hash",
                gate.blocking_set_hash.into_pyobject(py)?.into_any(),
            ),
            ("audits", PyTuple::new(py, identities)?.into_any()),
        ],
    )
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(build_audit_gate_identity, module)?)?;
    Ok(())
}
