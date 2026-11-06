//! Parsed MODEL header columns and audits built into the Python contract objects.

use std::sync::OnceLock;

use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::sync::PyOnceLock;
use pyo3::types::{PyBool, PyDict, PyDictMethods, PyTuple};
use pyo3::{FromPyObject, IntoPyObject, IntoPyObjectExt};
use sqlbuild_discovery::model_files::models::{NativeAudit, NativeColumn};
use sqlbuild_discovery::models::LineColumnSpan;
use sqlbuild_model_config::header_metadata::constants::AUDITS_KEY;
use sqlbuild_model_config::header_metadata::models::{
    ParsedAudit, ParsedColumn, ParsedThresholds, ThresholdBound,
};
use sqlbuild_model_config::types::AuthoredNode;
use sqlbuild_sqltext::compiler::models::AuthoredValue;

use crate::bindings::_helpers::discovery::model_file_record::NativeModelFile;
use crate::bindings::_helpers::model_config::authored_nodes::PyNode;
use crate::bindings::_helpers::model_config::config_nodes::{
    PyConfigNode, config_node, config_object,
};
use crate::bindings::_helpers::model_config::contract_classes::contract_classes;
use crate::bindings::_helpers::model_config::header_audits::{
    AuditInstance, HeaderAudits, authored_audit,
};
use crate::bindings::_helpers::model_config::typed_config::node_text;
use crate::bindings::_helpers::records::construct::{new_record, record_class};
use crate::bindings::_helpers::records::fields::all_specs;
use crate::bindings::_helpers::records::fields::field;
use crate::bindings::_helpers::records::ids::RecordId;
use crate::bindings::_helpers::sqltext::authored_values::authored_value_to_python;

/// `SchemaColumn` fields in declaration order, the order its objects are built positionally in.
const SCHEMA_COLUMN_FIELDS: [&str; 8] = [
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

/// A parsed leaf projected only when a public contract needs its Python value.
pub(crate) trait HeaderValue: AuthoredNode {
    fn object<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>>;

    fn retained(&self, py: Python<'_>) -> PyResult<PyConfigNode> {
        config_node(&self.object(py)?)
    }

    fn exact_text(&self, py: Python<'_>) -> PyResult<String> {
        self.object(py)?.extract()
    }
}

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

/// Native column fields; a Python record is constructed only for public consumers.
pub(crate) struct HeaderColumn {
    fields: ColumnFields,
    class: Py<PyAny>,
    audits: Option<HeaderAudits>,
    projection: PyOnceLock<Py<PyAny>>,
}

/// What a column's Python record is built from.
enum ColumnFields {
    /// The record's field values, in declaration order.
    Python([Py<PyAny>; 8]),
    /// A header column read natively, with what its location and audits are built with.
    Native(Box<NativeColumnSpec>),
    /// Only the record itself, built by Python.
    Projected,
}

/// A natively parsed header column and what its Python record is built with.
pub(crate) struct NativeColumnSpec {
    pub(crate) column: NativeColumn,
    /// The model file whose relative path the column's location names, and the location.
    pub(crate) location: Option<(Py<NativeModelFile>, LineColumnSpan)>,
}

impl HeaderColumn {
    /// Retype a builtin column without discarding its retained audit specs.
    pub(crate) fn enum_type(
        mut self,
        py: Python<'_>,
        scalar_type: Bound<'_, PyAny>,
        generated: Option<ParsedAudit<PyConfigNode>>,
    ) -> PyResult<Self> {
        if self.audits.is_none()
            && let ColumnFields::Native(spec) = &self.fields
        {
            let audits = spec
                .column
                .audits
                .iter()
                .map(|audit| authored_audit(py, audit))
                .collect::<PyResult<Vec<_>>>()?;
            let classes: ContractClasses<'_> = contract_classes(py)?.extract()?;
            self.audits = Some(HeaderAudits::new(
                py,
                &classes,
                &audits,
                &native_location(py, spec)?,
            )?);
        }
        if let Some(audits) = &mut self.audits {
            if let Some(generated) = generated {
                audits.push_native(generated);
            }
            match &mut self.fields {
                ColumnFields::Native(spec) => spec.column.column_type = scalar_type.extract()?,
                ColumnFields::Python(values) => values[1] = scalar_type.unbind(),
                ColumnFields::Projected => {
                    return Err(PyRuntimeError::new_err(
                        "enum column has no retained fields",
                    ));
                }
            }
            self.projection = PyOnceLock::new();
            return Ok(self);
        }
        Err(PyRuntimeError::new_err(
            "enum column has no retained audits",
        ))
    }

    pub(crate) fn retains_audits(&self) -> bool {
        self.audits.is_some() || matches!(self.fields, ColumnFields::Native(_))
    }

    pub(crate) fn audit_instances<'py>(
        &self,
        py: Python<'py>,
    ) -> PyResult<Vec<AuditInstance<'py>>> {
        if let Some(audits) = &self.audits {
            return Ok(audits.instances(py));
        }
        if let ColumnFields::Native(spec) = &self.fields {
            let audits = spec
                .column
                .audits
                .iter()
                .map(|audit| authored_audit(py, audit))
                .collect::<PyResult<Vec<_>>>()?;
            let classes: ContractClasses<'_> = contract_classes(py)?.extract()?;
            return Ok(
                HeaderAudits::new(py, &classes, &audits, &native_location(py, spec)?)?
                    .instances(py),
            );
        }
        self.field(py, "audits")?
            .try_iter()?
            .map(|value| value.map(AuditInstance::Python))
            .collect()
    }

    pub(crate) fn replaced<'py>(
        class: &Bound<'py, PyAny>,
        values: [Bound<'py, PyAny>; 8],
        keyword_order: [usize; 8],
    ) -> PyResult<Self> {
        if class.is(&record_class(class.py(), RecordId::SchemaColumn)?) {
            return Self::new(class, values);
        }
        let fields = PyDict::new(class.py());
        for index in keyword_order {
            fields.set_item(SCHEMA_COLUMN_FIELDS[index], &values[index])?;
        }
        Self::from_python(class.call((), Some(&fields))?)
    }

    pub(crate) fn field<'py>(&self, py: Python<'py>, name: &str) -> PyResult<Bound<'py, PyAny>> {
        let index = || {
            SCHEMA_COLUMN_FIELDS
                .iter()
                .position(|field| *field == name)
                .ok_or_else(|| PyRuntimeError::new_err("unknown column field"))
        };
        if let Some(projected) = self.projection.get(py) {
            return field(projected.bind(py), name);
        }
        if name == AUDITS_KEY
            && let Some(audits) = &self.audits
        {
            return Ok(audits.project(py)?.into_any());
        }
        match &self.fields {
            ColumnFields::Python(values) => Ok(values[index()?].bind(py).clone()),
            ColumnFields::Native(spec) => native_value(py, spec, index()?),
            ColumnFields::Projected => field(&self.project(py)?, name),
        }
    }

    pub(crate) fn project<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        self.projection
            .get_or_try_init(py, || {
                let mut values: [Bound<'py, PyAny>; 8] = match &self.fields {
                    ColumnFields::Python(values) => {
                        values.each_ref().map(|value| value.bind(py).clone())
                    }
                    ColumnFields::Native(spec) => native_values(py, spec)?,
                    ColumnFields::Projected => {
                        return Err(PyRuntimeError::new_err("missing column projection"));
                    }
                };
                if let Some(audits) = &self.audits {
                    values[5] = audits.project(py)?.into_any();
                }
                contract_object(self.class.bind(py), values).map(Bound::unbind)
            })
            .map(|value| value.bind(py).clone())
    }

    pub(crate) fn from_python(value: Bound<'_, PyAny>) -> PyResult<Self> {
        let py = value.py();
        let projection = PyOnceLock::new();
        let class = value.get_type().into_any().unbind();
        let _ = projection.set(py, value.unbind());
        Ok(Self {
            fields: ColumnFields::Projected,
            class,
            audits: None,
            projection,
        })
    }

    pub(crate) fn new<'py>(
        class: &Bound<'py, PyAny>,
        values: [Bound<'py, PyAny>; 8],
    ) -> PyResult<Self> {
        if !class.is(&record_class(class.py(), RecordId::SchemaColumn)?) {
            return Self::from_python(contract_object(class, values)?);
        }
        checked_field_orders()?;
        Ok(Self {
            fields: ColumnFields::Python(values.map(Bound::unbind)),
            class: class.clone().unbind(),
            audits: None,
            projection: PyOnceLock::new(),
        })
    }

    /// A natively parsed column, whose Python record is built only when Python reads it.
    pub(crate) fn native(class: &Bound<'_, PyAny>, spec: NativeColumnSpec) -> PyResult<Self> {
        let py: Python<'_> = class.py();
        if !class.is(&record_class(py, RecordId::SchemaColumn)?) {
            return Self::from_python(contract_object(class, native_values(py, &spec)?)?);
        }
        checked_field_orders()?;
        Ok(Self {
            fields: ColumnFields::Native(Box::new(spec)),
            class: class.clone().unbind(),
            audits: None,
            projection: PyOnceLock::new(),
        })
    }
}

impl HeaderColumn {
    /// The original native column, if no retained audit edits changed its public contract.
    pub(crate) fn native_spec(&self) -> Option<&NativeColumnSpec> {
        if self.audits.is_some() {
            return None;
        }
        match &self.fields {
            ColumnFields::Native(spec) => Some(spec),
            ColumnFields::Python(_) | ColumnFields::Projected => None,
        }
    }
}

/// The `SchemaColumn` a native column projects to when its location names `location`'s path.
pub(crate) fn native_column_record<'py>(
    py: Python<'py>,
    column: &NativeColumn,
    location: Option<(Bound<'py, PyAny>, LineColumnSpan)>,
) -> PyResult<Bound<'py, PyAny>> {
    let location: Bound<'py, PyAny> = match location {
        Some((path, span)) => location_record(py, path, span)?,
        None => py.None().into_bound(py),
    };
    contract_object(
        &record_class(py, RecordId::SchemaColumn)?,
        column_values(py, column, location)?,
    )
}

/// A native column's record fields, in declaration order.
fn native_values<'py>(
    py: Python<'py>,
    spec: &NativeColumnSpec,
) -> PyResult<[Bound<'py, PyAny>; 8]> {
    column_values(py, &spec.column, native_location(py, spec)?)
}

/// A native column's record fields at `location`, in declaration order.
fn column_values<'py>(
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
fn native_value<'py>(
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
fn native_location<'py>(py: Python<'py>, spec: &NativeColumnSpec) -> PyResult<Bound<'py, PyAny>> {
    match &spec.location {
        Some((model_file, span)) => {
            location_record(py, model_file.get().path_objects(py)?.1, *span)
        }
        None => Ok(py.None().into_bound(py)),
    }
}

/// `SourceLocation(path, line, column, end_line, end_column)`.
fn location_record<'py>(
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

/// Retained native columns and their shared, lazy public tuple.
pub(crate) struct HeaderColumns {
    pub(crate) specs: Vec<HeaderColumn>,
    projection: PyOnceLock<Py<PyTuple>>,
}

impl HeaderColumns {
    pub(crate) fn new(specs: Vec<HeaderColumn>) -> Self {
        Self {
            specs,
            projection: PyOnceLock::new(),
        }
    }

    pub(crate) fn project<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyTuple>> {
        self.projection
            .get_or_try_init(py, || {
                let columns = self
                    .specs
                    .iter()
                    .map(|column| column.project(py))
                    .collect::<PyResult<Vec<_>>>()?;
                PyTuple::new(py, columns).map(Bound::unbind)
            })
            .map(|value| value.bind(py).clone())
    }
}

/// The Python classes the parsed metadata becomes, read from a Python mapping.
#[derive(FromPyObject)]
#[pyo3(from_item_all)]
pub(crate) struct ContractClasses<'py> {
    pub(crate) schema_column: Bound<'py, PyAny>,
    pub(crate) schema_audit_instance: Bound<'py, PyAny>,
    pub(crate) schema_dynamic_column_family: Bound<'py, PyAny>,
    pub(crate) source_location: Bound<'py, PyAny>,
    measurement_thresholds: Bound<'py, PyAny>,
    measurement_threshold_bound: Bound<'py, PyAny>,
    pub(crate) severities: Bound<'py, PyDict>,
    threshold_operators: Bound<'py, PyDict>,
}

impl ContractClasses<'_> {
    /// Whether these are the classes native header parsing builds by default.
    pub(crate) fn uses_default_classes(&self) -> PyResult<bool> {
        let py: Python<'_> = self.schema_column.py();
        let default: ContractClasses<'_> = contract_classes(py)?.extract()?;
        Ok(self.schema_column.is(&default.schema_column)
            && self
                .schema_audit_instance
                .is(&default.schema_audit_instance)
            && self.source_location.is(&default.source_location)
            && self
                .measurement_thresholds
                .is(&default.measurement_thresholds)
            && self
                .measurement_threshold_bound
                .is(&default.measurement_threshold_bound)
            && self.severities.is(&default.severities)
            && self.threshold_operators.is(&default.threshold_operators))
    }
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

pub(crate) fn audit_tuple<'py>(
    py: Python<'py>,
    classes: &ContractClasses<'py>,
    parsed: &[ParsedAudit<PyNode<'py>>],
) -> PyResult<Bound<'py, PyTuple>> {
    PyTuple::new(py, audit_instances(py, classes, parsed)?)
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

/// `audit.name or audit.definition_name` of the instance `audit` builds.
pub(crate) fn parsed_audit_identity<N: HeaderValue>(
    py: Python<'_>,
    audit: &ParsedAudit<N>,
) -> PyResult<String> {
    if let Some(name) = &audit.name
        && name.object(py)?.is_truthy()?
    {
        return name.exact_text(py);
    }
    audit.definition_name.exact_text(py)
}

/// `class(*values)`: the same record as keywords bind, given in `POSITIONAL_RECORDS` order.
fn contract_object<'py, const N: usize>(
    class: &Bound<'py, PyAny>,
    values: [Bound<'py, PyAny>; N],
) -> PyResult<Bound<'py, PyAny>> {
    checked_field_orders()?;
    class.call1(PyTuple::new(class.py(), values)?)
}

/// Fail when a positionally built record no longer declares its fields in the expected order.
fn checked_field_orders() -> PyResult<()> {
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

fn optional<'py, N: HeaderValue>(
    py: Python<'py>,
    value: Option<&N>,
) -> PyResult<Bound<'py, PyAny>> {
    Ok(value
        .map(|node| node.object(py))
        .transpose()?
        .unwrap_or_else(|| py.None().into_bound(py)))
}

fn schema_column<'py>(
    py: Python<'py>,
    classes: &ContractClasses<'py>,
    column: &ParsedColumn<PyNode<'py>>,
    locations: &Bound<'py, PyDict>,
) -> PyResult<Bound<'py, PyAny>> {
    native_schema_column(py, classes, column, locations)?.project(py)
}

pub(crate) fn native_schema_column<'py, N: HeaderValue>(
    py: Python<'py>,
    classes: &ContractClasses<'py>,
    column: &ParsedColumn<N>,
    locations: &Bound<'py, PyDict>,
) -> PyResult<HeaderColumn> {
    let location = locations
        .get_item(column.name.object(py)?)?
        .unwrap_or_else(|| py.None().into_bound(py));
    let audits = HeaderAudits::new(py, classes, &column.audits, &location)?;
    let projected_audits = if classes
        .schema_column
        .is(&record_class(py, RecordId::SchemaColumn)?)
    {
        PyTuple::empty(py)
    } else {
        audits.project(py)?
    };
    let mut result = HeaderColumn::new(
        &classes.schema_column,
        [
            column.name.object(py)?,
            optional(py, column.column_type.as_ref())?,
            optional(py, column.nullable.as_ref())?,
            optional(py, column.description.as_ref())?,
            PyDict::new(py).into_any(),
            projected_audits.into_any(),
            location,
            optional(py, column.migrate_from.as_ref())?,
        ],
    )?;
    if !matches!(result.fields, ColumnFields::Projected) {
        result.audits = Some(audits);
    }
    Ok(result)
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

/// `MeasurementThresholds` of `parsed`, or `None`.
pub(crate) fn thresholds<'py>(
    py: Python<'py>,
    classes: &ContractClasses<'py>,
    parsed: Option<&ParsedThresholds>,
) -> PyResult<Bound<'py, PyAny>> {
    let Some(parsed) = parsed else {
        return Ok(py.None().into_bound(py));
    };
    contract_object(
        &classes.measurement_thresholds,
        [
            threshold_bound(py, classes, parsed.warn)?,
            threshold_bound(py, classes, parsed.error)?,
        ],
    )
}

fn threshold_bound<'py>(
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
