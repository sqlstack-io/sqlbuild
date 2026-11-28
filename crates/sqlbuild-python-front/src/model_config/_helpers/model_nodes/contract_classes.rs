//! The Python classes native header and YAML parsing build contract objects as.

use crate::model_config::main::model_nodes::contract_classes::contract_classes;
use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::sync::PyOnceLock;
use pyo3::types::{PyDict, PyDictMethods};
use sqlbuild_host::entries::models::FacadeHost;
use sqlbuild_python_base::native_host::main::host_entry::host_entry;
use sqlbuild_python_base::records::main::construct::record_class::record_class;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::RecordId;

const CONTRACT_CLASSES: [(&str, RecordId); 6] = [
    ("schema_column", RecordId::SchemaColumn),
    ("schema_audit_instance", RecordId::SchemaAuditInstance),
    (
        "schema_dynamic_column_family",
        RecordId::SchemaDynamicColumnFamily,
    ),
    ("source_location", RecordId::SourceLocation),
    ("measurement_thresholds", RecordId::MeasurementThresholds),
    (
        "measurement_threshold_bound",
        RecordId::MeasurementThresholdBound,
    ),
];

const YAML_ENTRY_CLASSES: [(&str, RecordId); 8] = [
    ("schema_seed_entry", RecordId::SchemaSeedEntry),
    ("seed_csv_settings", RecordId::SeedCsvSettings),
    ("source_entry", RecordId::SourceEntry),
    ("source_column_entry", RecordId::SourceColumnEntry),
    ("source_freshness_config", RecordId::SourceFreshnessConfig),
    (
        "source_freshness_age_policy",
        RecordId::SourceFreshnessAgePolicy,
    ),
    (
        "source_freshness_strategy",
        RecordId::SourceFreshnessStrategy,
    ),
    (
        "source_freshness_value_kind",
        RecordId::SourceFreshnessValueKind,
    ),
];

pub(crate) static CONTRACT_CLASS_TABLE: PyOnceLock<Py<PyDict>> = PyOnceLock::new();

pub(crate) static YAML_ENTRY_CLASS_TABLE: PyOnceLock<Py<PyDict>> = PyOnceLock::new();

pub(crate) fn built_contract_classes(py: Python<'_>) -> PyResult<Bound<'_, PyDict>> {
    let classes: Bound<'_, PyDict> = PyDict::new(py);
    for (key, class) in CONTRACT_CLASSES {
        classes.set_item(key, record_class(py, class)?)?;
    }
    classes.set_item(
        "severities",
        members_by_value(&host_entry(py, FacadeHost::AuditSeverity)?)?,
    )?;
    classes.set_item(
        "threshold_operators",
        members_by_value(&host_entry(py, FacadeHost::ThresholdOperator)?)?,
    )?;
    Ok(classes)
}

pub(crate) fn built_yaml_entry_classes(py: Python<'_>) -> PyResult<Bound<'_, PyDict>> {
    let classes: Bound<'_, PyDict> = PyDict::new(py);
    classes.set_item("contracts", contract_classes(py)?)?;
    for (key, class) in YAML_ENTRY_CLASSES {
        classes.set_item(key, record_class(py, class)?)?;
    }
    Ok(classes)
}

/// `{member.value: member for member in enum}`.
fn members_by_value<'py>(enumeration: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyDict>> {
    let members: Bound<'py, PyDict> = PyDict::new(enumeration.py());
    for member in enumeration.try_iter()? {
        let member: Bound<'py, PyAny> = member?;
        members.set_item(native_field(&member, "value")?, member)?;
    }
    Ok(members)
}
