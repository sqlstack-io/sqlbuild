//! Create, read and append SQL of migration state tables, and stored rows decoded to events.

use crate::state_rows::main::event_rows::date_time_typed::date_time_typed;
use crate::state_rows::main::migrations::layout::layout;
use pyo3::exceptions::{PyKeyError, PyValueError};
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyErr, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::{PyDict, PyDictMethods, PyFrozenSet, PyString, PyTuple};
use sqlbuild_state::events::constants::TIMESTAMP_COLUMNS;
use sqlbuild_state::events::main::column_aliases_json::column_aliases_json;
use sqlbuild_state::events::main::decoded_column_aliases::decoded_column_aliases;
use sqlbuild_state::events::main::decoded_grants::decoded_grants;
use sqlbuild_state::events::main::event_exists_sql::event_exists_sql;
use sqlbuild_state::events::main::event_read_sql::event_read_sql;
use sqlbuild_state::events::main::grants_json::grants_json;
use sqlbuild_state::events::main::readable_columns::readable_columns;

use crate::cursor::constants::DATETIME_MODULE;
use crate::state_rows::_helpers::event_rows::events::typed_insert_sql;
use crate::state_rows::_helpers::migrations::migration_objects::{qualified_table, state_error};
use crate::state_rows::_helpers::tables::state_tables::state_table_create_sql;
use crate::state_rows::models::{EventTable, EventTableLayout};
use sqlbuild_python_base::boundary::main::imported::imported;
use sqlbuild_python_base::boundary::models::Request;
use sqlbuild_python_base::records::main::construct::enum_member::enum_member;
use sqlbuild_python_base::records::main::construct::new_record::new_record;
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::RecordId;

/// DDL that creates one migration state table when it is missing.
#[pyfunction(name = "render_migration_state_create_sql")]
#[pyo3(signature = (**request))]
pub(crate) fn create_sql<'py>(request: Option<&Bound<'py, PyDict>>) -> PyResult<Bound<'py, PyAny>> {
    let request = Request::new("render_migration_state_create_sql", request)?;
    let py: Python<'py> = request.py();
    let layout: EventTableLayout = layout(&request.get("kind")?.extract::<String>()?)?;
    let table: String = qualified_table(
        &layout,
        &request.get("render_qualified_name")?,
        (request.get("database")?, request.get("schema")?),
    )?;
    let column_types: Bound<'py, PyDict> = PyDict::new(py);
    for column in layout.columns {
        let member: &str = if TIMESTAMP_COLUMNS.contains(column) {
            "TEXT_TIMESTAMP"
        } else {
            "STRING"
        };
        column_types.set_item(
            *column,
            enum_member(py, RecordId::StateSqlValueType, member)?,
        )?;
    }
    let columns: Bound<'py, PyAny> = PyTuple::new(py, layout.columns)?.into_any();
    let sql: String = state_table_create_sql(
        &table,
        (
            &columns,
            column_types.as_any(),
            PyFrozenSet::empty(py)?.as_any(),
        ),
        &request.get("render_framework_type")?,
    )?;
    Ok(PyString::new(py, &sql).into_any())
}

/// The SQL reading every stored event of one schema, and the columns it selects.
pub(crate) fn read_statement<'py>(
    layout: &EventTableLayout,
    render_qualified_name: &Bound<'py, PyAny>,
    location: (Bound<'py, PyAny>, Bound<'py, PyAny>),
    stored: Option<&[String]>,
) -> PyResult<(String, Vec<String>)> {
    let columns: Vec<&str> = match stored {
        Some(stored) => readable_columns(layout.columns, stored),
        None => layout.columns.to_vec(),
    };
    let table: String = qualified_table(layout, render_qualified_name, location)?;
    Ok((
        event_read_sql(&table, &columns),
        columns.into_iter().map(str::to_owned).collect(),
    ))
}

fn optional<'py>(value: Bound<'py, PyAny>, attribute: &str) -> PyResult<Bound<'py, PyAny>> {
    if value.is_none() {
        Ok(value)
    } else {
        native_field(&value, attribute)
    }
}

fn text<'py>(py: Python<'py>, value: Option<String>) -> Bound<'py, PyAny> {
    match value {
        Some(value) => PyString::new(py, &value).into_any(),
        None => py.None().into_bound(py),
    }
}

fn row_values<'py>(
    table: EventTable,
    event: &Bound<'py, PyAny>,
) -> PyResult<Vec<Bound<'py, PyAny>>> {
    let py: Python<'py> = event.py();
    let field = |name: &str| native_field(event, name);
    let relation = |name: &str| -> PyResult<[Bound<'py, PyAny>; 3]> {
        let relation: Bound<'py, PyAny> = native_field(event, name)?;
        Ok([
            native_field(&relation, "database")?,
            native_field(&relation, "schema")?,
            native_field(&relation, "name")?,
        ])
    };
    let mut values: Vec<Bound<'py, PyAny>> = vec![field("event_id")?, field("target_name")?];
    match table {
        EventTable::Model => {
            values.push(field("origin_model")?);
            values.extend(relation("origin")?);
            values.push(field("destination_model")?);
            values.extend(relation("destination")?);
            values.push(field("origin_version_hash")?);
        }
        EventTable::Column => {
            values.push(field("model_name")?);
            values.extend(relation("relation")?);
            values.push(field("origin_column")?);
            values.push(field("destination_column")?);
        }
        EventTable::OldName => {
            values.push(native_field(&field("event_type")?, "value")?);
            values.push(field("migration_event_id")?);
            values.push(field("destination_model")?);
            values.extend(relation("old")?);
            values.extend(relation("new")?);
            values.push(field("view_retention")?);
            values.push(field("archive_name")?);
            let aliases: Vec<(String, String)> = field("column_aliases")?.extract()?;
            let encoded = column_aliases_json(&aliases).map_err(json_error)?;
            values.push(text(py, encoded));
            let grants: Option<Vec<String>> = field("grants_copied")?.extract()?;
            values.push(text(
                py,
                grants_json(grants.as_deref()).map_err(json_error)?,
            ));
            values.push(field("view_sql")?);
            values.push(field("expires_at")?);
            values.push(optional(field("drop_reason")?, "value")?);
            values.push(field("run_id")?);
            values.push(field("created_at")?);
            return Ok(values);
        }
    }
    values.push(native_field(&field("discovery")?, "value")?);
    values.push(native_field(&field("decision")?, "value")?);
    values.push(field("run_id")?);
    values.push(field("created_at")?);
    Ok(values)
}

fn json_error(error: sqlbuild_core::json::errors::JsonEmitError) -> PyErr {
    PyValueError::new_err(format!("{error:?}"))
}

/// The idempotency lookup and the insert that append one event to its schema's state table.
pub(crate) fn write_statements<'py>(
    layout: &EventTableLayout,
    event: &Bound<'py, PyAny>,
    render_qualified_name: &Bound<'py, PyAny>,
) -> PyResult<(String, String)> {
    let py: Python<'py> = event.py();
    let relation: Bound<'py, PyAny> = event.getattr(layout.relation_attribute)?;
    let schema: Bound<'py, PyAny> = native_field(&relation, "schema")?;
    if schema.is_none() {
        return Err(state_error(py, layout.event_message));
    }
    let table: String = qualified_table(
        layout,
        render_qualified_name,
        (native_field(&relation, "database")?, schema),
    )?;
    let event_id: String = native_field(event, "event_id")?.extract()?;
    let values: Vec<Bound<'py, PyAny>> = row_values(layout.table, event)?;
    let insert: String = typed_insert_sql(py, &table, (layout.columns, &values))?;
    Ok((event_exists_sql(&table, &event_id), insert))
}

/// One stored row keyed by the columns its query selected.
struct StoredRow<'a, 'py> {
    columns: &'a [String],
    values: Vec<Bound<'py, PyAny>>,
}

impl<'py> StoredRow<'_, 'py> {
    fn get(&self, column: &str) -> Option<&Bound<'py, PyAny>> {
        self.columns
            .iter()
            .position(|name| name == column)
            .and_then(|index| self.values.get(index))
    }

    fn required(&self, column: &str) -> PyResult<Bound<'py, PyAny>> {
        self.get(column)
            .cloned()
            .ok_or_else(|| PyKeyError::new_err(column.to_owned()))
    }

    fn text(&self, column: &str) -> PyResult<Bound<'py, PyAny>> {
        Ok(self.required(column)?.str()?.into_any())
    }

    fn optional_text(&self, column: &str) -> PyResult<Option<Bound<'py, PyAny>>> {
        match self.get(column) {
            Some(value) if !value.is_none() => Ok(Some(value.str()?.into_any())),
            _ => Ok(None),
        }
    }

    fn nullable(&self, py: Python<'py>, column: &str) -> PyResult<Bound<'py, PyAny>> {
        Ok(self
            .optional_text(column)?
            .unwrap_or_else(|| py.None().into_bound(py)))
    }

    fn relation(&self, py: Python<'py>, prefix: &str) -> PyResult<Bound<'py, PyAny>> {
        new_record_with(
            py,
            RecordId::MigrationRelation,
            &[
                (
                    "database",
                    self.nullable(py, &format!("{prefix}_database"))?,
                ),
                ("schema", self.nullable(py, &format!("{prefix}_schema"))?),
                ("name", self.text(&format!("{prefix}_name"))?),
            ],
        )
    }
}

/// `decode_event_timestamp`: a stored timestamp as an aware datetime, naive values read as UTC.
fn timestamp<'py>(raw: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = raw.py();
    let parsed: Bound<'py, PyAny> = if date_time_typed(raw)? {
        raw.clone()
    } else {
        imported(py, DATETIME_MODULE, "datetime")?.call_method1("fromisoformat", (raw.str()?,))?
    };
    if !native_field(&parsed, "tzinfo")?.is_none() {
        return Ok(parsed);
    }
    let utc: Bound<'py, PyDict> = PyDict::new(py);
    utc.set_item("tzinfo", imported(py, DATETIME_MODULE, "UTC")?)?;
    parsed.call_method("replace", (), Some(&utc))
}

fn enumerated<'py>(
    py: Python<'py>,
    class: RecordId,
    value: Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    new_record(py, class, (value,), None)
}

fn known_member<'py>(
    py: Python<'py>,
    class: RecordId,
    value: Bound<'py, PyAny>,
) -> PyResult<Option<Bound<'py, PyAny>>> {
    match enumerated(py, class, value) {
        Ok(member) => Ok(Some(member)),
        Err(error) if error.is_instance_of::<PyValueError>(py) => Ok(None),
        Err(error) => Err(error),
    }
}

fn model_event<'py>(py: Python<'py>, row: &StoredRow<'_, 'py>) -> PyResult<Bound<'py, PyAny>> {
    let created_at: Bound<'py, PyAny> = timestamp(&row.required("created_at")?)?;
    let version_hash: Bound<'py, PyAny> = row.required("origin_version_hash")?;
    let version_hash: Bound<'py, PyAny> = if version_hash.is_truthy()? {
        version_hash.str()?.into_any()
    } else {
        PyString::new(py, "").into_any()
    };
    new_record_with(
        py,
        RecordId::MigrationEvent,
        &[
            ("event_id", row.text("event_id")?),
            ("target_name", row.nullable(py, "target_name")?),
            ("origin_model", row.nullable(py, "origin_model")?),
            ("origin", row.relation(py, "origin")?),
            ("destination_model", row.text("destination_model")?),
            ("destination", row.relation(py, "destination")?),
            ("origin_version_hash", version_hash),
            (
                "discovery",
                enumerated(py, RecordId::MigrationDiscovery, row.text("discovery")?)?,
            ),
            (
                "decision",
                enumerated(py, RecordId::MigrationDecision, row.text("decision")?)?,
            ),
            ("run_id", row.text("run_id")?),
            ("created_at", created_at),
        ],
    )
}

fn column_event<'py>(py: Python<'py>, row: &StoredRow<'_, 'py>) -> PyResult<Bound<'py, PyAny>> {
    new_record_with(
        py,
        RecordId::ColumnMigrationEvent,
        &[
            ("event_id", row.text("event_id")?),
            ("target_name", row.nullable(py, "target_name")?),
            ("model_name", row.text("model_name")?),
            ("relation", row.relation(py, "relation")?),
            ("origin_column", row.text("origin_column")?),
            ("destination_column", row.text("destination_column")?),
            (
                "discovery",
                enumerated(py, RecordId::MigrationDiscovery, row.text("discovery")?)?,
            ),
            (
                "decision",
                enumerated(py, RecordId::ColumnMigrationDecision, row.text("decision")?)?,
            ),
            ("run_id", row.text("run_id")?),
            ("created_at", timestamp(&row.required("created_at")?)?),
        ],
    )
}

fn stored_text(row: &StoredRow<'_, '_>, column: &str) -> PyResult<Option<String>> {
    row.optional_text(column)?
        .map(|value| value.extract())
        .transpose()
}

fn old_name_event<'py>(
    py: Python<'py>,
    row: &StoredRow<'_, 'py>,
) -> PyResult<Option<Bound<'py, PyAny>>> {
    let raw_type: Bound<'py, PyAny> = match row.get("event_type") {
        Some(value) => value.str()?.into_any(),
        None => PyString::new(py, "None").into_any(),
    };
    let Some(event_type) = known_member(py, RecordId::OldNameViewEventType, raw_type)? else {
        return Ok(None);
    };
    let drop_reason: Option<Bound<'py, PyAny>> = match row.optional_text("drop_reason")? {
        Some(raw) => known_member(py, RecordId::OldNameViewDropReason, raw)?,
        None => None,
    };
    let none = || py.None().into_bound(py);
    let expires_at: Bound<'py, PyAny> = match row.get("expires_at") {
        Some(raw) if !raw.is_none() => timestamp(raw)?,
        _ => none(),
    };
    let aliases: Vec<(String, String)> =
        decoded_column_aliases(stored_text(row, "column_aliases")?.as_deref());
    let grants: Bound<'py, PyAny> =
        match decoded_grants(stored_text(row, "grants_copied")?.as_deref()) {
            Some(grants) => PyTuple::new(py, grants)?.into_any(),
            None => none(),
        };
    let fields: [(&str, Bound<'py, PyAny>); 16] = [
        ("event_id", row.text("event_id")?),
        ("target_name", row.nullable(py, "target_name")?),
        ("event_type", event_type),
        ("migration_event_id", row.text("migration_event_id")?),
        ("destination_model", row.text("destination_model")?),
        ("old", row.relation(py, "old")?),
        ("new", row.relation(py, "new")?),
        ("run_id", row.text("run_id")?),
        ("created_at", timestamp(&row.required("created_at")?)?),
        ("view_retention", row.nullable(py, "view_retention")?),
        ("archive_name", row.nullable(py, "archive_name")?),
        ("column_aliases", PyTuple::new(py, aliases)?.into_any()),
        ("expires_at", expires_at),
        ("drop_reason", drop_reason.unwrap_or_else(none)),
        ("grants_copied", grants),
        ("view_sql", row.nullable(py, "view_sql")?),
    ];
    new_record_with(py, RecordId::OldNameViewEvent, &fields).map(Some)
}

/// Stored rows of the selected `columns` as events; rows this version cannot read are skipped.
pub(crate) fn decoded_events<'py>(
    table: EventTable,
    rows: &Bound<'py, PyAny>,
    columns: &[String],
) -> PyResult<Bound<'py, PyTuple>> {
    let py: Python<'py> = rows.py();
    let mut events: Vec<Bound<'py, PyAny>> = Vec::new();
    for row in rows.try_iter()? {
        let values: Vec<Bound<'py, PyAny>> = row?.try_iter()?.collect::<PyResult<_>>()?;
        let stored = StoredRow { columns, values };
        let decoded: Option<Bound<'py, PyAny>> = match table {
            EventTable::Model => Some(model_event(py, &stored)?),
            EventTable::Column => match column_event(py, &stored) {
                Ok(event) => Some(event),
                Err(error) if error.is_instance_of::<PyValueError>(py) => None,
                Err(error) => return Err(error),
            },
            EventTable::OldName => old_name_event(py, &stored)?,
        };
        events.extend(decoded);
    }
    PyTuple::new(py, events)
}
