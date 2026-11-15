//! Janitor relation keys, schema keys, times and the executor records planning builds.

use std::collections::HashSet;

use pyo3::prelude::{Bound, IntoPyObject, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyString, PyTuple};
use sqlbuild_analysis::janitor_planning::models::ArchiveTimestamp;
use sqlbuild_host::entries::models::ExecutorHost;

use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::boundary::main::imported::imported;
use sqlbuild_python_base::native_host::main::host_entry::host_entry;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

const DATETIME_MODULE: &str = "datetime";

/// `(database, schema)` as spelled.
pub(crate) type SchemaKey = (Option<String>, Option<String>);
/// Case-insensitive relation identity.
pub(crate) type Folded = (Option<String>, Option<String>, String);

/// One physical relation identity, compared case-insensitively but rendered as spelled.
#[derive(Clone, Debug)]
pub(crate) struct Key {
    pub(crate) database: Option<String>,
    pub(crate) schema: Option<String>,
    pub(crate) name: String,
}

impl Key {
    /// The key of an object with `database`, `schema` and `name` attributes.
    pub(crate) fn of(item: &Bound<'_, PyAny>) -> PyResult<Self> {
        Ok(Self {
            database: native_field(item, "database")?.extract()?,
            schema: native_field(item, "schema")?.extract()?,
            name: native_field(item, "name")?.extract()?,
        })
    }

    pub(crate) fn folded(&self) -> Folded {
        (
            fold(self.database.as_deref()),
            fold(self.schema.as_deref()),
            self.name.to_lowercase(),
        )
    }

    pub(crate) fn display_name(&self) -> String {
        [
            self.database.as_deref(),
            self.schema.as_deref(),
            Some(&self.name),
        ]
        .into_iter()
        .flatten()
        .collect::<Vec<&str>>()
        .join(".")
    }

    /// The executor's `JanitorRelationKey`.
    pub(crate) fn to_python<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        record(
            py,
            ExecutorHost::JanitorRelationKey,
            &[
                ("database", optional(py, self.database.as_deref())?),
                ("schema", optional(py, self.schema.as_deref())?),
                ("name", text(py, &self.name)),
            ],
        )
    }
}

pub(crate) fn fold(value: Option<&str>) -> Option<String> {
    value.map(str::to_lowercase)
}

pub(crate) fn folded_schema((database, schema): &SchemaKey) -> SchemaKey {
    (fold(database.as_deref()), fold(schema.as_deref()))
}

/// Schema keys ordered by `(database or "", schema or "")`, as Python sorts them.
pub(crate) fn sorted_schemas(keys: &[SchemaKey]) -> Vec<SchemaKey> {
    let mut sorted: Vec<SchemaKey> = keys.to_vec();
    sorted.sort_by(|left, right| schema_order(left).cmp(&schema_order(right)));
    sorted
}

fn schema_order((database, schema): &SchemaKey) -> (&str, &str) {
    (
        database.as_deref().unwrap_or(""),
        schema.as_deref().unwrap_or(""),
    )
}

/// One spelling per case-insensitive schema, keeping the first spelling met.
pub(crate) fn dedupe_schema_keys(keys: impl IntoIterator<Item = SchemaKey>) -> Vec<SchemaKey> {
    let mut seen: HashSet<SchemaKey> = HashSet::new();
    let mut kept: Vec<SchemaKey> = Vec::new();
    for key in keys {
        if seen.insert(folded_schema(&key)) {
            kept.push(key);
        }
    }
    kept
}

/// `keys` followed by each of `extra` whose exact spelling is not already present.
pub(crate) fn union_exact(
    keys: Vec<SchemaKey>,
    extra: impl IntoIterator<Item = SchemaKey>,
) -> Vec<SchemaKey> {
    let mut joined: Vec<SchemaKey> = keys;
    for key in extra {
        if !joined.contains(&key) {
            joined.push(key);
        }
    }
    joined
}

pub(crate) fn text<'py>(py: Python<'py>, value: &str) -> Bound<'py, PyAny> {
    PyString::new(py, value).into_any()
}

pub(crate) fn optional<'py>(py: Python<'py>, value: Option<&str>) -> PyResult<Bound<'py, PyAny>> {
    Ok(value.into_pyobject(py)?.into_any())
}

pub(crate) fn integer(py: Python<'_>, value: i64) -> PyResult<Bound<'_, PyAny>> {
    Ok(value.into_pyobject(py)?.into_any())
}

pub(crate) fn tuple<'py>(
    py: Python<'py>,
    items: impl IntoIterator<Item = Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    let items: Vec<Bound<'py, PyAny>> = items.into_iter().collect();
    Ok(PyTuple::new(py, items)?.into_any())
}

/// `sqlbuild.executor.janitor.models.<class_name>(**pairs)`.
pub(crate) fn record<'py>(
    py: Python<'py>,
    class_name: ExecutorHost,
    pairs: &[(&str, Bound<'py, PyAny>)],
) -> PyResult<Bound<'py, PyAny>> {
    call_with(&host_entry(py, class_name)?, pairs)
}

fn utc(py: Python<'_>) -> PyResult<Bound<'_, PyAny>> {
    imported(py, DATETIME_MODULE, "UTC")
}

/// `datetime.now(UTC)`.
pub(crate) fn now_utc(py: Python<'_>) -> PyResult<Bound<'_, PyAny>> {
    imported(py, DATETIME_MODULE, "datetime")?.call_method1("now", (utc(py)?,))
}

/// `timedelta(days=days)`.
pub(crate) fn days<'py>(py: Python<'py>, days: i64) -> PyResult<Bound<'py, PyAny>> {
    call_with(
        &imported(py, DATETIME_MODULE, "timedelta")?,
        &[("days", days.into_pyobject(py)?.into_any())],
    )
}

/// The value as an aware UTC datetime; naive values are taken as UTC.
pub(crate) fn aware_utc<'py>(value: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = value.py();
    if native_field(value, "tzinfo")?.is_none() {
        return call_with(&native_field(value, "replace")?, &[("tzinfo", utc(py)?)]);
    }
    value.call_method1("astimezone", (utc(py)?,))
}

/// The latest known creation or alteration time of a relation.
pub(crate) fn relation_age_timestamp<'py>(
    relation: &Bound<'py, PyAny>,
) -> PyResult<Option<Bound<'py, PyAny>>> {
    let mut latest: Option<Bound<'py, PyAny>> = None;
    for field in ["created_at", "last_altered_at"] {
        let value: Bound<'py, PyAny> = native_field(relation, field)?;
        if value.is_none() {
            continue;
        }
        let aware: Bound<'py, PyAny> = aware_utc(&value)?;
        latest = match latest {
            Some(current) if !aware.gt(&current)? => Some(current),
            _ => Some(aware),
        };
    }
    Ok(latest)
}

/// The UTC second-resolution time embedded in archive names.
pub(crate) fn archive_timestamp<'py>(value: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = value.py();
    call_with(
        &native_field(&aware_utc(value)?, "replace")?,
        &[("microsecond", 0_i64.into_pyobject(py)?.into_any())],
    )
}

/// The calendar fields of a datetime.
pub(crate) fn timestamp_fields(value: &Bound<'_, PyAny>) -> PyResult<ArchiveTimestamp> {
    Ok(ArchiveTimestamp {
        year: native_field(value, "year")?.extract()?,
        month: native_field(value, "month")?.extract()?,
        day: native_field(value, "day")?.extract()?,
        hour: native_field(value, "hour")?.extract()?,
        minute: native_field(value, "minute")?.extract()?,
        second: native_field(value, "second")?.extract()?,
    })
}

/// An aware UTC datetime at the given calendar fields.
pub(crate) fn utc_datetime(py: Python<'_>, value: ArchiveTimestamp) -> PyResult<Bound<'_, PyAny>> {
    call_with(
        &imported(py, DATETIME_MODULE, "datetime")?,
        &[
            ("year", value.year.into_pyobject(py)?.into_any()),
            ("month", value.month.into_pyobject(py)?.into_any()),
            ("day", value.day.into_pyobject(py)?.into_any()),
            ("hour", value.hour.into_pyobject(py)?.into_any()),
            ("minute", value.minute.into_pyobject(py)?.into_any()),
            ("second", value.second.into_pyobject(py)?.into_any()),
            ("tzinfo", utc(py)?),
        ],
    )
}
