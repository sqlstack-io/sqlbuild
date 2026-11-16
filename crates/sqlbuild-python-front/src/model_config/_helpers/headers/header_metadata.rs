//! Parsed MODEL header columns and audits built into the Python contract objects.

use crate::model_config::main::config::config_object::config_object;
use crate::model_config::main::headers::native_schema_column::native_schema_column;
use crate::model_config::main::headers::thresholds::thresholds;
use crate::model_config::main::model_nodes::contract_classes::contract_classes;
use crate::model_config::models::ContractClasses;
use crate::model_config::models::NativeColumnSpec;
use crate::model_config::types::HeaderValue;
use std::sync::OnceLock;

use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyBool, PyDict, PyDictMethods, PyTuple};
use pyo3::{IntoPyObject, IntoPyObjectExt};
use sqlbuild_discovery::model_files::models::{NativeAudit, NativeColumn};
use sqlbuild_discovery::models::LineColumnSpan;
use sqlbuild_model_config::header_metadata::models::{ParsedAudit, ParsedColumn, ThresholdBound};
use sqlbuild_sqltext::compiler::models::AuthoredValue;

use crate::model_config::_helpers::config::typed_config::node_text;
use crate::model_config::models::PyNode;
use crate::model_config::types::PyConfigNode;
use crate::sqltext::main::authored_value_to_python::authored_value_to_python;
use sqlbuild_python_base::records::main::construct::new_record::new_record;
use sqlbuild_python_base::records::main::fields::all_specs::all_specs;
use sqlbuild_python_base::records::types::RecordId;

/// `SchemaColumn` fields in declaration order, the order its objects are built positionally in.
pub(crate) const SCHEMA_COLUMN_FIELDS: [&str; 8] = [
    "name",
    "type",
    "nullable",
    "description",
    "meta",
    "audits",
    "location",
    "migrate_from",
];
/// `SchemaAuditInstance` fields in declaration order.
const AUDIT_INSTANCE_FIELDS: [&str; 12] = [
    "definition_name",
    "arguments",
    "name",
    "description",
    "severity",
    "run_scope",
    "always_run",
    "thresholds",
    "minimum_samples",
    "evidence_limit",
    "meta",
    "location",
];
/// `MeasurementThresholds` fields in declaration order.
const THRESHOLDS_FIELDS: [&str; 2] = ["warn", "error"];
/// `MeasurementThresholdBound` fields in declaration order.
const THRESHOLD_BOUND_FIELDS: [&str; 4] = ["operator", "limit", "lower", "upper"];
/// Each positionally built record with the field order its values are given in.
const POSITIONAL_RECORDS: [(&str, &[&str]); 4] = [
    ("SchemaColumn", &SCHEMA_COLUMN_FIELDS),
    ("SchemaAuditInstance", &AUDIT_INSTANCE_FIELDS),
    ("MeasurementThresholds", &THRESHOLDS_FIELDS),
    ("MeasurementThresholdBound", &THRESHOLD_BOUND_FIELDS),
];

impl HeaderValue for PyNode<'_> {
    fn object<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        Ok(self.0.as_unbound().bind(py).clone())
    }
}

impl HeaderValue for &PyConfigNode {
    fn object<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        config_object(py, self)
    }

    fn exact_text(&self, py: Python<'_>) -> PyResult<String> {
        node_text(py, self)
    }

    fn retained(&self, _py: Python<'_>) -> PyResult<PyConfigNode> {
        Ok((*self).clone())
    }
}

impl HeaderValue for PyConfigNode {
    fn object<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        config_object(py, self)
    }

    fn exact_text(&self, py: Python<'_>) -> PyResult<String> {
        node_text(py, self)
    }

    fn retained(&self, _py: Python<'_>) -> PyResult<PyConfigNode> {
        Ok(self.clone())
    }
}

/// What a column's Python record is built from.
pub(crate) enum ColumnFields {
    /// The record's field values, in declaration order.
    Python([Py<PyAny>; 8]),
    /// A header column read natively, with what its location and audits are built with.
    Native(Box<NativeColumnSpec>),
    /// Only the record itself, built by Python.
    Projected,
}

/// A native column's record fields, in declaration order.
pub(crate) fn native_values<'py>(
    py: Python<'py>,
    spec: &NativeColumnSpec,
) -> PyResult<[Bound<'py, PyAny>; 8]> {
    column_values(py, &spec.column, native_location(py, spec)?)
}

/// A native column's record fields at `location`, in declaration order.
pub(crate) fn column_values<'py>(
    py: Python<'py>,
    column: &NativeColumn,
    location: Bound<'py, PyAny>,
) -> PyResult<[Bound<'py, PyAny>; 8]> {
    let audits: Bound<'py, PyAny> = native_audits(py, column, &location)?;
    Ok([
        column.name.as_str().into_bound_py_any(py)?,
        column.column_type.as_deref().into_bound_py_any(py)?,
        column.nullable.into_bound_py_any(py)?,
        column.description.as_deref().into_bound_py_any(py)?,
        PyDict::new(py).into_any(),
        audits,
        location,
        column.migrate_from.as_deref().into_bound_py_any(py)?,
    ])
}

/// The native column's record field at `index` of `SCHEMA_COLUMN_FIELDS`.
pub(crate) fn native_value<'py>(
    py: Python<'py>,
    spec: &NativeColumnSpec,
    index: usize,
) -> PyResult<Bound<'py, PyAny>> {
    let column: &NativeColumn = &spec.column;
    match SCHEMA_COLUMN_FIELDS[index] {
        "name" => column.name.as_str().into_bound_py_any(py),
        "type" => column.column_type.as_deref().into_bound_py_any(py),
        "nullable" => column.nullable.into_bound_py_any(py),
        "description" => column.description.as_deref().into_bound_py_any(py),
        "meta" => Ok(PyDict::new(py).into_any()),
        "audits" => native_audits(py, column, &native_location(py, spec)?),
        "location" => native_location(py, spec),
        _ => column.migrate_from.as_deref().into_bound_py_any(py),
    }
}

/// The column's `SourceLocation`, or `None`.
pub(crate) fn native_location<'py>(
    py: Python<'py>,
    spec: &NativeColumnSpec,
) -> PyResult<Bound<'py, PyAny>> {
    match &spec.location {
        Some((model_file, span)) => {
            location_record(py, model_file.get().path_objects(py)?.1, *span)
        }
        None => Ok(py.None().into_bound(py)),
    }
}

/// `SourceLocation(path, line, column, end_line, end_column)`.
pub(crate) fn location_record<'py>(
    py: Python<'py>,
    path: Bound<'py, PyAny>,
    span: LineColumnSpan,
) -> PyResult<Bound<'py, PyAny>> {
    new_record(
        py,
        RecordId::SourceLocation,
        (path, span.line, span.column, span.end_line, span.end_column),
        None,
    )
}

/// The column's audits as a tuple of `SchemaAuditInstance`s at `location`.
fn native_audits<'py>(
    py: Python<'py>,
    column: &NativeColumn,
    location: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let classes: ContractClasses<'py> = contract_classes(py)?.extract()?;
    let audits: Vec<Bound<'py, PyAny>> = column
        .audits
        .iter()
        .map(|audit| native_audit_instance(py, &classes, audit, location))
        .collect::<PyResult<_>>()?;
    Ok(PyTuple::new(py, audits)?.into_any())
}

/// A native audit as the `SchemaAuditInstance` the Python parse builds.
fn native_audit_instance<'py>(
    py: Python<'py>,
    classes: &ContractClasses<'py>,
    audit: &NativeAudit,
    location: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let arguments = PyDict::new(py);
    for (key, value) in &audit.arguments {
        arguments.set_item(key, authored_value_to_python(py, value)?)?;
    }
    let severity = match &audit.severity {
        Some(severity) => classes
            .severities
            .get_item(severity)?
            .unwrap_or_else(|| py.None().into_bound(py)),
        None => py.None().into_bound(py),
    };
    let count = |value: Option<&AuthoredValue>| -> PyResult<Bound<'py, PyAny>> {
        match value {
            Some(value) => Ok(authored_value_to_python(py, value)?.into_bound(py)),
            None => Ok(py.None().into_bound(py)),
        }
    };
    contract_object(
        &classes.schema_audit_instance,
        [
            audit.definition_name.as_str().into_bound_py_any(py)?,
            arguments.into_any(),
            audit.name.as_deref().into_bound_py_any(py)?,
            audit.description.as_deref().into_bound_py_any(py)?,
            severity,
            audit.run_scope.as_deref().into_bound_py_any(py)?,
            audit.always_run.unwrap_or(false).into_bound_py_any(py)?,
            thresholds(py, classes, audit.thresholds.as_ref())?,
            count(audit.minimum_samples.as_ref())?,
            count(audit.evidence_limit.as_ref())?,
            count(audit.meta.as_ref())?,
            location.clone(),
        ],
    )
}

pub(crate) fn column_tuple<'py>(
    py: Python<'py>,
    classes: &ContractClasses<'py>,
    parsed: &[ParsedColumn<PyNode<'py>>],
    locations: &Bound<'py, PyDict>,
) -> PyResult<Bound<'py, PyTuple>> {
    PyTuple::new(py, schema_columns(py, classes, parsed, locations)?)
}

/// The parsed columns as `SchemaColumn` objects, in declaration order.
pub(crate) fn schema_columns<'py>(
    py: Python<'py>,
    classes: &ContractClasses<'py>,
    parsed: &[ParsedColumn<PyNode<'py>>],
    locations: &Bound<'py, PyDict>,
) -> PyResult<Vec<Bound<'py, PyAny>>> {
    let mut columns: Vec<Bound<'py, PyAny>> = Vec::with_capacity(parsed.len());
    for column in parsed {
        columns.push(schema_column(py, classes, column, locations)?);
    }
    Ok(columns)
}

/// The parsed audits as `SchemaAuditInstance` objects without a location, in order.
pub(crate) fn audit_instances<'py, N: HeaderValue>(
    py: Python<'py>,
    classes: &ContractClasses<'py>,
    parsed: &[ParsedAudit<N>],
) -> PyResult<Vec<Bound<'py, PyAny>>> {
    let no_location: Bound<'py, PyAny> = py.None().into_bound(py);
    let mut audits: Vec<Bound<'py, PyAny>> = Vec::with_capacity(parsed.len());
    for audit in parsed {
        audits.push(audit_instance(py, classes, audit, &no_location)?);
    }
    Ok(audits)
}

/// `class(*values)`: the same record as keywords bind, given in `POSITIONAL_RECORDS` order.
pub(crate) fn contract_object<'py, const N: usize>(
    class: &Bound<'py, PyAny>,
    values: [Bound<'py, PyAny>; N],
) -> PyResult<Bound<'py, PyAny>> {
    checked_field_orders()?;
    class.call1(PyTuple::new(class.py(), values)?)
}

/// Fail when a positionally built record no longer declares its fields in the expected order.
pub(crate) fn checked_field_orders() -> PyResult<()> {
    static MISMATCH: OnceLock<Option<&'static str>> = OnceLock::new();
    match MISMATCH.get_or_init(mismatched_record) {
        Some(record) => Err(PyRuntimeError::new_err(format!(
            "native record {record} does not declare the fields header metadata builds it with"
        ))),
        None => Ok(()),
    }
}

/// The first positionally built record whose registered fields differ from its expected order.
fn mismatched_record() -> Option<&'static str> {
    for (record, fields) in POSITIONAL_RECORDS {
        let mut declared: bool = false;
        for spec in all_specs() {
            if spec.name == record {
                declared = spec.fields.len() == fields.len()
                    && spec
                        .fields
                        .iter()
                        .zip(fields)
                        .all(|(field, expected)| field.name == *expected);
                break;
            }
        }
        if !declared {
            return Some(record);
        }
    }
    None
}

pub(crate) fn optional<'py, N: HeaderValue>(
    py: Python<'py>,
    value: Option<&N>,
) -> PyResult<Bound<'py, PyAny>> {
    Ok(value
        .map(|node| node.object(py))
        .transpose()?
        .unwrap_or_else(|| py.None().into_bound(py)))
}

pub(crate) fn schema_column<'py>(
    py: Python<'py>,
    classes: &ContractClasses<'py>,
    column: &ParsedColumn<PyNode<'py>>,
    locations: &Bound<'py, PyDict>,
) -> PyResult<Bound<'py, PyAny>> {
    native_schema_column(py, classes, column, locations)?.project(py)
}

pub(crate) fn audit_instance<'py, N: HeaderValue>(
    py: Python<'py>,
    classes: &ContractClasses<'py>,
    audit: &ParsedAudit<N>,
    location: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let arguments = PyDict::new(py);
    for (key, value) in &audit.arguments {
        arguments.set_item(key.object(py)?, value.object(py)?)?;
    }
    let severity = match &audit.severity {
        Some(severity) => classes
            .severities
            .get_item(severity.object(py)?)?
            .unwrap_or_else(|| py.None().into_bound(py)),
        None => py.None().into_bound(py),
    };
    let always_run = match &audit.always_run {
        Some(node) => node.object(py)?,
        None => PyBool::new(py, false).to_owned().into_any(),
    };
    contract_object(
        &classes.schema_audit_instance,
        [
            audit.definition_name.object(py)?,
            arguments.into_any(),
            optional(py, audit.name.as_ref())?,
            optional(py, audit.description.as_ref())?,
            severity,
            optional(py, audit.run_scope.as_ref())?,
            always_run,
            thresholds(py, classes, audit.thresholds.as_ref())?,
            optional(py, audit.minimum_samples.as_ref())?,
            optional(py, audit.evidence_limit.as_ref())?,
            optional(py, audit.meta.as_ref())?,
            location.clone(),
        ],
    )
}

pub(crate) fn threshold_bound<'py>(
    py: Python<'py>,
    classes: &ContractClasses<'py>,
    bound: Option<ThresholdBound>,
) -> PyResult<Bound<'py, PyAny>> {
    let Some(bound) = bound else {
        return Ok(py.None().into_bound(py));
    };
    let (operator, limit, lower, upper) = match bound {
        ThresholdBound::Below(limit) => ("below", Some(limit), None, None),
        ThresholdBound::Above(limit) => ("above", Some(limit), None, None),
        ThresholdBound::Outside(lower, upper) => ("outside", None, Some(lower), Some(upper)),
    };
    let operator = classes
        .threshold_operators
        .get_item(operator)?
        .unwrap_or_else(|| py.None().into_bound(py));
    contract_object(
        &classes.measurement_threshold_bound,
        [
            operator,
            limit.into_pyobject(py)?.into_any(),
            lower.into_pyobject(py)?.into_any(),
            upper.into_pyobject(py)?.into_any(),
        ],
    )
}
