//! Construction, comparison and display of native record field values.

use crate::native_host::models::Dialect;
use crate::native_host::types::Syntax;
use std::collections::{HashMap, HashSet};
use std::sync::{Mutex, OnceLock};

use pyo3::IntoPyObjectExt;
use pyo3::exceptions::{PyAttributeError, PyRuntimeError, PyTypeError};
use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyErr, PyModule, PyResult, Python};
use pyo3::pyclass::CompareOp;
use pyo3::sync::PyOnceLock;
use pyo3::types::{
    PyBool, PyDict, PyDictMethods, PyFrozenSet, PyString, PyStringMethods, PyTuple, PyTupleMethods,
    PyType, PyTypeMethods,
};

use sqlbuild_discovery::model_files::main::model_file_stem::model_file_stem;
use sqlbuild_rules::project_tree::main::pure_path_posix::pure_path_posix;

use crate::boundary::main::imported::imported;
use crate::native_host::main::host_entry::host_entry;
use crate::records::_helpers::native_classes::behaviours::behaviours;
use crate::records::_helpers::native_classes::construct::{enum_member, new_record};
use crate::records::_helpers::native_classes::getters::getter_field;
use crate::records::constants::{
    CURRENT_DIRECTORY_PATH, ENUMS, FROZEN_INSTANCE_ERROR, NATIVE_MODULE, RECORD_SPEC_ATTRIBUTE,
    RECORDS,
};
use crate::records::models::{EnumSpec, FieldDefault, NativeRecord, RecordSpec};
use crate::records::types::{ClassProperty, RecordFinish};

/// How a test run treats `field` falling back to `getattr` on a native class: unset outside tests.
pub(crate) const NATIVE_FIELD_STRICT_ENV_VAR: &str = "SQLBUILD_NATIVE_FIELD_STRICT";

/// The string enum member attributes answered natively.
const ENUM_VALUE_ATTRIBUTE: &str = "value";
const ENUM_NAME_ATTRIBUTE: &str = "name";

/// Native string enums by class object, for `.value` and `.name` reads answered from their members.
static ENUM_CLASSES: PyOnceLock<HashMap<usize, &'static EnumSpec>> = PyOnceLock::new();

/// Each registered record class's spec, by class object.
static REGISTERED_SPECS: PyOnceLock<HashMap<usize, &'static RecordSpec>> = PyOnceLock::new();

/// Every registered record spec, in registration order.
pub(crate) fn all_specs() -> impl Iterator<Item = &'static RecordSpec> {
    RECORDS.iter().flat_map(|group| group.iter())
}

/// Keep each registered record class's spec, so records of that exact class find it natively.
pub(crate) fn register_specs(
    py: Python<'_>,
    classes: HashMap<usize, &'static RecordSpec>,
) -> PyResult<()> {
    REGISTERED_SPECS.get_or_init(py, || classes);
    Ok(())
}

/// The spec registered for `cls`, read from its class attribute.
pub(crate) fn record_spec(cls: &Bound<'_, PyType>) -> PyResult<&'static RecordSpec> {
    if let Some(spec) = REGISTERED_SPECS
        .get(cls.py())
        .and_then(|specs| specs.get(&(cls.as_ptr() as usize)))
    {
        return Ok(spec);
    }
    let Ok(index) = cls.getattr(RECORD_SPEC_ATTRIBUTE) else {
        return Err(PyTypeError::new_err(format!(
            "{} cannot be instantiated directly",
            cls.qualname()?
        )));
    };
    let index: usize = index.extract()?;
    all_specs()
        .nth(index)
        .ok_or_else(|| PyTypeError::new_err("unknown native record class"))
}

/// A declared field of a native record read from its values; any other attribute, or object, by `getattr`.
#[track_caller]
pub(crate) fn field<'py>(value: &Bound<'py, PyAny>, name: &str) -> PyResult<Bound<'py, PyAny>> {
    if let Ok(cell) = value.cast::<NativeRecord>() {
        let record: &NativeRecord = cell.get();
        if let Some(found) = record
            .spec
            .fields
            .iter()
            .position(|field| field.name == name)
            .and_then(|index| record.values.get(index))
        {
            return Ok(found.bind(value.py()).clone());
        }
        if let Some(&(property, cached_name)) = record_properties().get(&(record.spec.name, name)) {
            return property_value(cell, property, cached_name)
                .map(|found| found.into_bound(value.py()));
        }
    } else if let Some(found) = getter_field(value, name) {
        return found;
    } else if let Some(found) = enum_member_field(value, name) {
        return found;
    } else if let Ok(dialect) = value.cast::<Dialect>() {
        return dialect.syntax(name);
    }
    strict_fallback(value, name)?;
    value.getattr(name)
}

/// `getattr(value, name, None)` of a native record, `None` when `value` is not one.
pub(crate) fn record_attribute_opt<'py>(
    value: &Bound<'py, PyAny>,
    name: &str,
) -> Option<PyResult<Option<Bound<'py, PyAny>>>> {
    let Ok(cell) = value.cast::<NativeRecord>() else {
        return None;
    };
    if name.starts_with('_') {
        return None;
    }
    let record: &NativeRecord = cell.get();
    if let Some(found) = record
        .spec
        .fields
        .iter()
        .position(|field| field.name == name)
        .and_then(|index| record.values.get(index))
    {
        return Some(Ok(Some(found.bind(value.py()).clone())));
    }
    match record_properties().get(&(record.spec.name, name)) {
        Some(&(property, cached_name)) => Some(
            property_value(cell, property, cached_name)
                .map(|found| Some(found.into_bound(value.py()))),
        ),
        None if behaviours()
            .methods
            .iter()
            .flat_map(|group| group.iter())
            .any(|(class, method, _)| *class == record.spec.name && *method == name) =>
        {
            None
        }
        None => Some(Ok(None)),
    }
}

/// A strict-mode setting: record fallbacks to a file, or fail on any fallback missing from an allowlist.
enum StrictFields {
    Record(String, Mutex<HashSet<String>>),
    Enforce(Result<HashSet<String>, String>),
}

fn strict_fields() -> Option<&'static StrictFields> {
    static STRICT: OnceLock<Option<StrictFields>> = OnceLock::new();
    STRICT
        .get_or_init(|| {
            let setting: String = std::env::var_os(NATIVE_FIELD_STRICT_ENV_VAR)?
                .to_string_lossy()
                .into_owned();
            if let Some(path) = setting.strip_prefix("record:") {
                return Some(StrictFields::Record(
                    path.to_owned(),
                    Mutex::new(HashSet::new()),
                ));
            }
            let path: &str = setting.strip_prefix("enforce:")?;
            Some(StrictFields::Enforce(
                std::fs::read_to_string(path)
                    .map(|listed| allowlist_entries(&listed))
                    .map_err(|error| format!("cannot read native field allowlist {path}: {error}")),
            ))
        })
        .as_ref()
}

/// The non-comment entries of an allowlist file.
fn allowlist_entries(listed: &str) -> HashSet<String> {
    let mut entries: HashSet<String> = HashSet::new();
    for line in listed.lines().map(str::trim) {
        if !line.is_empty() && !line.starts_with('#') {
            let _ = entries.insert(line.to_owned());
        }
    }
    entries
}

/// In strict test runs, record or reject a `getattr` fallback on one of SQLBuild's own native classes.
#[track_caller]
fn strict_fallback(value: &Bound<'_, PyAny>, name: &str) -> PyResult<()> {
    let Some(strict) = strict_fields() else {
        return Ok(());
    };
    let class: Bound<'_, PyType> = value.get_type();
    if class.module()?.to_cow()? != NATIVE_MODULE {
        return Ok(());
    }
    let caller: &std::panic::Location<'_> = std::panic::Location::caller();
    let qualname: Bound<'_, PyString> = class.qualname()?;
    let entry: String = format!("{}\t{}.{name}", caller.file(), qualname.to_cow()?);
    match strict {
        StrictFields::Enforce(Err(error)) => Err(PyRuntimeError::new_err(error.clone())),
        StrictFields::Enforce(Ok(allowed)) if !allowed.contains(&entry) => {
            value.py().check_signals()?;
            Err(PyRuntimeError::new_err(format!(
                "native field read fell back to Python: {}",
                entry.replace('\t', " ")
            )))
        }
        StrictFields::Enforce(Ok(_)) => Ok(()),
        StrictFields::Record(path, seen) => {
            let new: bool = seen
                .lock()
                .map(|mut held| held.insert(entry.clone()))
                .unwrap_or(false);
            if new
                && let Ok(mut file) = std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(path)
            {
                let _ = std::io::Write::write_all(&mut file, format!("{entry}\n").as_bytes());
            }
            Ok(())
        }
    }
}

/// Record properties by class and attribute name, each with its cache key when cached.
type PropertyTable = HashMap<(&'static str, &'static str), (ClassProperty, Option<&'static str>)>;

/// A record property's function by class and attribute name, with its cache key when cached.
fn record_properties() -> &'static PropertyTable {
    static TABLE: OnceLock<PropertyTable> = OnceLock::new();
    TABLE.get_or_init(|| {
        let mut table: PropertyTable = HashMap::new();
        for &(class, name, property) in behaviours()
            .properties
            .iter()
            .flat_map(|group| group.iter())
        {
            let _ = table.insert((class, name), (property, None));
        }
        for &(class, name, property) in behaviours()
            .cached_properties
            .iter()
            .flat_map(|group| group.iter())
        {
            let _ = table.insert((class, name), (property, Some(name)));
        }
        table
    })
}

/// A record property computed as its descriptor computes it, cached ones computed once.
fn property_value(
    record: &Bound<'_, NativeRecord>,
    property: ClassProperty,
    cached_name: Option<&'static str>,
) -> PyResult<Py<PyAny>> {
    let Some(cached_name) = cached_name else {
        return property(record.as_any());
    };
    if let Some(value) = cached_value(record, cached_name)? {
        return Ok(value);
    }
    let value: Py<PyAny> = property(record.as_any())?;
    store_cached(record, cached_name, &value)?;
    Ok(value)
}

/// `member.value` or `member.name` of a native string enum, from its declared members.
fn enum_member_field<'py>(
    value: &Bound<'py, PyAny>,
    name: &str,
) -> Option<PyResult<Bound<'py, PyAny>>> {
    let wants_value: bool = match name {
        ENUM_VALUE_ATTRIBUTE => true,
        ENUM_NAME_ATTRIBUTE => false,
        _ => return None,
    };
    let py: Python<'py> = value.py();
    let classes: &HashMap<usize, &'static EnumSpec> =
        match ENUM_CLASSES.get_or_try_init(py, || enum_classes(py)) {
            Ok(classes) => classes,
            Err(error) => return Some(Err(error)),
        };
    let spec: &EnumSpec = classes.get(&(value.get_type().as_ptr() as usize))?;
    let Ok(member) = value.cast::<PyString>() else {
        return None;
    };
    let text: String = match member.to_str() {
        Ok(text) => text.to_owned(),
        Err(error) => return Some(Err(error)),
    };
    let (member_name, member_value) = spec
        .members
        .iter()
        .find(|(_, declared)| *declared == text)?;
    let answer: &str = if wants_value {
        member_value
    } else {
        member_name
    };
    Some(Ok(PyString::new(py, answer).into_any()))
}

fn enum_classes(py: Python<'_>) -> PyResult<HashMap<usize, &'static EnumSpec>> {
    let module: Bound<'_, PyModule> = py.import(NATIVE_MODULE)?;
    let mut classes: HashMap<usize, &'static EnumSpec> = HashMap::new();
    for spec in ENUMS.iter().flat_map(|group| group.iter()) {
        let class: Bound<'_, PyAny> = module.getattr(spec.name)?;
        let _ = classes.insert(class.as_ptr() as usize, spec);
    }
    Ok(classes)
}

/// `mapping.get(key)`: a dict is read through the C API.
pub(crate) fn mapping_get<'py, K: pyo3::IntoPyObject<'py>>(
    mapping: &Bound<'py, PyAny>,
    key: K,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = mapping.py();
    let key: Bound<'py, PyAny> = key.into_bound_py_any(py)?;
    match mapping.cast_exact::<PyDict>() {
        Ok(dict) => Ok(dict
            .get_item(&key)?
            .unwrap_or_else(|| py.None().into_bound(py))),
        Err(_) => mapping.call_method1("get", (key,)),
    }
}

/// `mapping.get(key, default)`: a dict is read through the C API.
pub(crate) fn mapping_get_or<'py, K: pyo3::IntoPyObject<'py>, D: pyo3::IntoPyObject<'py>>(
    mapping: &Bound<'py, PyAny>,
    key: K,
    default: D,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = mapping.py();
    let key: Bound<'py, PyAny> = key.into_bound_py_any(py)?;
    if let Ok(dict) = mapping.cast_exact::<PyDict>() {
        if let Some(found) = dict.get_item(&key)? {
            return Ok(found);
        }
        return default.into_bound_py_any(py);
    }
    mapping.call_method1("get", (key, default.into_bound_py_any(py)?))
}

/// `mapping.items()` for iteration: a dict yields its `(key, value)` pairs through the C API.
pub(crate) fn mapping_items<'py>(mapping: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    match mapping.cast_exact::<PyDict>() {
        Ok(dict) => Ok(dict.items().into_any()),
        Err(_) => mapping.call_method0("items"),
    }
}

/// `mapping.values()` for iteration: a dict yields its values through the C API.
pub(crate) fn mapping_values<'py>(mapping: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    match mapping.cast_exact::<PyDict>() {
        Ok(dict) => Ok(dict.values().into_any()),
        Err(_) => mapping.call_method0("values"),
    }
}

/// `dataclasses.replace(value, **changes)`: a native record is rebuilt from its own values.
pub(crate) fn replaced<'py>(
    value: &Bound<'py, PyAny>,
    changes: &[(&str, Bound<'py, PyAny>)],
) -> PyResult<Bound<'py, PyAny>> {
    let keywords: Bound<'py, PyDict> = PyDict::new(value.py());
    for (name, changed) in changes {
        keywords.set_item(*name, changed)?;
    }
    replaced_keywords(value, &keywords)
}

/// `dataclasses.replace(value, **keywords)`: a native record is rebuilt from its own values.
pub(crate) fn replaced_keywords<'py>(
    value: &Bound<'py, PyAny>,
    keywords: &Bound<'py, PyDict>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = value.py();
    let keywords: Bound<'py, PyDict> = keywords.copy()?;
    let Ok(record) = value.cast::<NativeRecord>() else {
        return imported(py, "dataclasses", "replace")?.call((value,), Some(&keywords));
    };
    let record: &NativeRecord = record.get();
    for (field, current) in record.spec.fields.iter().zip(&record.values) {
        if !keywords.contains(field.name)? {
            keywords.set_item(field.name, current.bind(py))?;
        }
    }
    value.get_type().call((), Some(&keywords))
}

/// Field values bound from `args` and `kwargs` as a dataclass `__init__` binds them.
pub(crate) fn build_values(
    spec: &RecordSpec,
    args: &Bound<'_, PyTuple>,
    kwargs: Option<&Bound<'_, PyDict>>,
) -> PyResult<Vec<Py<PyAny>>> {
    let py: Python<'_> = args.py();
    let name: &str = spec.name;
    if args.len() > spec.fields.len() {
        return Err(PyTypeError::new_err(format!(
            "{name}.__init__() takes {} positional arguments but {} were given",
            spec.fields.len() + 1,
            args.len() + 1
        )));
    }
    if let Some(kwargs) = kwargs {
        for key in kwargs.keys() {
            let key: String = key.extract()?;
            if !spec.fields.iter().any(|field| field.name == key) {
                return Err(PyTypeError::new_err(format!(
                    "{name}.__init__() got an unexpected keyword argument '{key}'"
                )));
            }
        }
    }
    let mut values: Vec<Py<PyAny>> = Vec::with_capacity(spec.fields.len());
    for (index, field) in spec.fields.iter().enumerate() {
        let keyword: Option<Bound<'_, PyAny>> = match kwargs {
            Some(kwargs) => kwargs.get_item(field.name)?,
            None => None,
        };
        let positional: Option<Bound<'_, PyAny>> = if index < args.len() {
            Some(args.get_item(index)?)
        } else {
            None
        };
        let value: Bound<'_, PyAny> = match (positional, keyword) {
            (Some(_), Some(_)) => {
                return Err(PyTypeError::new_err(format!(
                    "{name}.__init__() got multiple values for argument '{}'",
                    field.name
                )));
            }
            (Some(value), None) | (None, Some(value)) => value,
            (None, None) => default_value(py, name, field.name, field.default)?,
        };
        values.push(value.unbind());
    }
    match record_finishers().get(name) {
        Some(finish) => finish(py, values),
        None => Ok(values),
    }
}

/// Each finished record's finisher, by class name.
fn record_finishers() -> &'static HashMap<&'static str, RecordFinish> {
    static FINISHERS: OnceLock<HashMap<&'static str, RecordFinish>> = OnceLock::new();
    FINISHERS.get_or_init(|| {
        behaviours()
            .finishers
            .iter()
            .flat_map(|group| group.iter())
            .map(|&(record, finish)| (record, finish))
            .collect()
    })
}

fn default_value<'py>(
    py: Python<'py>,
    record: &str,
    field: &str,
    default: FieldDefault,
) -> PyResult<Bound<'py, PyAny>> {
    match default {
        FieldDefault::Required => Err(PyTypeError::new_err(format!(
            "{record}.__init__() missing 1 required positional argument: '{field}'"
        ))),
        FieldDefault::None => Ok(py.None().into_bound(py)),
        FieldDefault::False => Ok(PyBool::new(py, false).to_owned().into_any()),
        FieldDefault::True => Ok(PyBool::new(py, true).to_owned().into_any()),
        FieldDefault::Int(value) => value.into_bound_py_any(py),
        FieldDefault::Text(value) => Ok(PyString::new(py, value).into_any()),
        FieldDefault::TextTuple(texts) => Ok(PyTuple::new(py, texts)?.into_any()),
        FieldDefault::TextFrozenset(texts) => Ok(PyFrozenSet::new(py, texts)?.into_any()),
        FieldDefault::EmptyTuple => Ok(PyTuple::empty(py).into_any()),
        FieldDefault::EmptyDict => Ok(PyDict::new(py).into_any()),
        FieldDefault::EmptyFrozenset => Ok(PyFrozenSet::empty(py)?.into_any()),
        FieldDefault::Member(module, class, member) => imported(py, module, class)?.getattr(member),
        FieldDefault::HostMember(class, member) => host_entry(py, class)?.getattr(member),
        FieldDefault::NativeMember(class, member) => enum_member(py, class, member),
        FieldDefault::NativeCall(class) => new_record(py, class, (), None),
        FieldDefault::Factory(factory) => factory(py),
        FieldDefault::CallText(module, name, text) => imported(py, module, name)?.call1((text,)),
    }
}

/// The values of the fields that take part in equality and hashing.
pub(crate) fn compared_values<'py>(
    record: &Bound<'py, NativeRecord>,
) -> PyResult<Bound<'py, PyTuple>> {
    let py: Python<'py> = record.py();
    let spec: &RecordSpec = record.get().spec;
    let values = spec
        .fields
        .iter()
        .zip(record.get().values.iter())
        .filter(|(field, _)| field.compared)
        .map(|(_, value)| value.clone_ref(py))
        .collect::<Vec<Py<PyAny>>>();
    PyTuple::new(py, values)
}

/// `Name(field=value!r, ...)` over the shown fields, as a dataclass repr reads.
pub(crate) fn repr_text(record: &Bound<'_, NativeRecord>) -> PyResult<String> {
    let py: Python<'_> = record.py();
    let cls: Bound<'_, PyType> = record.get_type();
    let spec: &RecordSpec = record.get().spec;
    let mut parts: Vec<String> = Vec::with_capacity(spec.fields.len());
    for (field, value) in spec.fields.iter().zip(record.get().values.iter()) {
        if field.shown {
            parts.push(format!("{}={}", field.name, value.bind(py).repr()?));
        }
    }
    Ok(format!("{}({})", cls.qualname()?, parts.join(", ")))
}

/// The `dataclasses.FrozenInstanceError` raised when assigning to a record field.
pub(crate) fn frozen_error(py: Python<'_>, name: &str) -> PyErr {
    let message: String = format!("cannot assign to field '{name}'");
    match imported(py, FROZEN_INSTANCE_ERROR.0, FROZEN_INSTANCE_ERROR.1)
        .and_then(|error| error.call1((message.as_str(),)))
    {
        Ok(error) => PyErr::from_value(error),
        Err(_) => PyAttributeError::new_err(message),
    }
}

/// `slf <op> other` over the compared fields, as an `order=True` dataclass compares.
pub(crate) fn ordered_comparison(
    slf: &Bound<'_, NativeRecord>,
    other: &Bound<'_, PyAny>,
    op: CompareOp,
) -> PyResult<Py<PyAny>> {
    let py: Python<'_> = slf.py();
    let name: &str = slf.get().spec.name;
    let ordered: bool = behaviours()
        .ordered
        .iter()
        .any(|group| group.contains(&name));
    if !ordered || !other.get_type().is(slf.get_type()) {
        return Ok(py.NotImplemented());
    }
    let other: &Bound<'_, NativeRecord> = other.cast::<NativeRecord>()?;
    compared_values(slf)?
        .as_any()
        .rich_compare(compared_values(other)?, op)?
        .into_py_any(py)
}

/// The cached value of `name` on `record`, if computed before.
pub(crate) fn cached_value(
    record: &Bound<'_, NativeRecord>,
    name: &str,
) -> PyResult<Option<Py<PyAny>>> {
    let py: Python<'_> = record.py();
    let cached = record
        .get()
        .cached
        .lock()
        .map_err(|_| PyRuntimeError::new_err("native record cache is poisoned"))?;
    Ok(cached
        .iter()
        .find(|(cached_name, _)| *cached_name == name)
        .map(|(_, value)| value.clone_ref(py)))
}

/// Keep `value` as the cached value of `name` on `record`.
pub(crate) fn store_cached(
    record: &Bound<'_, NativeRecord>,
    name: &'static str,
    value: &Py<PyAny>,
) -> PyResult<()> {
    let py: Python<'_> = record.py();
    let mut cached = record
        .get()
        .cached
        .lock()
        .map_err(|_| PyRuntimeError::new_err("native record cache is poisoned"))?;
    if !cached.iter().any(|(cached_name, _)| *cached_name == name) {
        cached.push((name, value.clone_ref(py)));
    }
    Ok(())
}

/// The text of a record's string field `name`.
pub(crate) fn field_text(value: &Bound<'_, PyAny>, name: &str) -> PyResult<String> {
    field(value, name)?.extract()
}

/// `path.as_posix()` of a `pathlib` path, spelled natively from its text.
pub(crate) fn path_posix(path: &Bound<'_, PyAny>) -> PyResult<String> {
    let text: String = path.str()?.to_string_lossy().into_owned();
    Ok(String::from_utf8_lossy(&pure_path_posix(std::path::Path::new(&text))).into_owned())
}

/// The value of the native string enum `enum_name`'s member `member`.
pub(crate) fn enum_member_value(enum_name: &str, member: &str) -> Option<&'static str> {
    ENUMS
        .iter()
        .flat_map(|group| group.iter())
        .find(|spec| spec.name == enum_name)?
        .members
        .iter()
        .find(|(name, _)| *name == member)
        .map(|(_, value)| *value)
}

/// `path.stem` of a `pathlib` path, spelled natively from its text.
pub(crate) fn path_stem(path: &Bound<'_, PyAny>) -> PyResult<String> {
    let posix: String = path_posix_text(path)?;
    if posix == CURRENT_DIRECTORY_PATH {
        return Ok(String::new());
    }
    Ok(model_file_stem(&posix).to_owned())
}

/// `path.as_posix()` of a `pathlib` path, its text with the platform separator spelled `/`.
pub(crate) fn path_posix_text(path: &Bound<'_, PyAny>) -> PyResult<String> {
    let text: Bound<'_, PyString> = path.str()?;
    Ok(text.to_str()?.replace(std::path::MAIN_SEPARATOR, "/"))
}

/// `{item.<attribute>: item for item in items}`.
pub(crate) fn by_attribute<'py>(
    items: &Bound<'py, PyAny>,
    attribute: &str,
) -> PyResult<Bound<'py, PyDict>> {
    let keyed: Bound<'py, PyDict> = PyDict::new(items.py());
    for item in items.try_iter()? {
        let item: Bound<'py, PyAny> = item?;
        keyed.set_item(field(&item, attribute)?, item)?;
    }
    Ok(keyed)
}
