//! Register the native record base, its field descriptor and every record class.

use pyo3::exceptions::{PyException, PyValueError};
use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyModule, PyModuleMethods, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods, PyString, PyTuple};

use crate::records::_helpers::native_classes::error_classes::{
    BoundErrorInitializer, ErrorInitializer, class_value,
};
use crate::records::constants::{
    BUILTIN_PARENT_PREFIX, BUILTINS_MODULE, DATACLASS_FIELD_KIND, DATACLASS_FIELDS_ATTRIBUTE,
    DATACLASSES_MODULE, ENUM_MODULE, ENUMS, ERROR_SPEC_ATTRIBUTE, ERRORS, EXCEPTION_PARENT,
    NATIVE_MODULE, RECORD_SPEC_ATTRIBUTE, STR_ENUM, VALUE_ERROR_PARENT,
};
use std::collections::HashMap;

use crate::records::_helpers::native_classes::behaviours::{self, behaviours};
use crate::records::_helpers::native_classes::fields::field as native_field;
use crate::records::_helpers::native_classes::fields::{all_specs, register_specs};
use crate::records::_helpers::native_classes::getters;
use crate::records::models::{
    BoundNativeMethod, EnumSpec, ErrorSpec, NativeMethod, NativeProperty, NativeRecord,
    RecordAttribute, RecordSpec,
};
use crate::records::models::{GetterEntry, RecordBehaviours};

pub(crate) fn register(
    module: &Bound<'_, PyModule>,
    getter_tables: &'static [&'static [GetterEntry]],
    record_behaviours: &'static RecordBehaviours,
) -> PyResult<()> {
    getters::install(getter_tables);
    behaviours::install(record_behaviours);
    let py: Python<'_> = module.py();
    module.add_class::<NativeRecord>()?;
    module.add_class::<RecordAttribute>()?;
    module.add_class::<NativeProperty>()?;
    module.add_class::<NativeMethod>()?;
    module.add_class::<BoundNativeMethod>()?;
    let base: Bound<'_, PyAny> = py.get_type::<NativeRecord>().into_any();
    let metaclass: Bound<'_, PyAny> = base.get_type().into_any();
    let str_enum: Bound<'_, PyAny> = py.import(ENUM_MODULE)?.getattr(STR_ENUM)?;
    for spec in ENUMS.iter().flat_map(|group| group.iter()) {
        module.add(spec.name, string_enum(&str_enum, spec)?)?;
    }
    module.add_class::<ErrorInitializer>()?;
    module.add_class::<BoundErrorInitializer>()?;
    for (index, spec) in ERRORS.iter().flat_map(|group| group.iter()).enumerate() {
        let parent: Bound<'_, PyAny> = match spec.parent {
            EXCEPTION_PARENT => py.get_type::<PyException>().into_any(),
            VALUE_ERROR_PARENT => py.get_type::<PyValueError>().into_any(),
            parent => match parent.strip_prefix(BUILTIN_PARENT_PREFIX) {
                Some(builtin) => py.import(BUILTINS_MODULE)?.getattr(builtin)?,
                None => module.getattr(parent)?,
            },
        };
        module.add(spec.name, error_class(&parent, (index, spec))?)?;
    }
    let dataclasses: Bound<'_, PyModule> = py.import(DATACLASSES_MODULE)?;
    let mut registered: HashMap<usize, &'static RecordSpec> = HashMap::new();
    for (index, spec) in all_specs().enumerate() {
        let class: Bound<'_, PyAny> = record_class(&metaclass, &base, (index, spec))?;
        let _ = registered.insert(class.as_ptr() as usize, spec);
        class.setattr(
            DATACLASS_FIELDS_ATTRIBUTE,
            dataclass_fields(&dataclasses, spec)?,
        )?;
        module.add(spec.name, class)?;
    }
    register_specs(py, registered)?;
    for (class, name, property) in behaviours()
        .properties
        .iter()
        .flat_map(|group| group.iter())
    {
        let descriptor: Py<NativeProperty> = Py::new(
            py,
            NativeProperty {
                property: *property,
                name,
                cached: false,
            },
        )?;
        module.getattr(*class)?.setattr(*name, descriptor)?;
    }
    for (class, name, property) in behaviours()
        .cached_properties
        .iter()
        .flat_map(|group| group.iter())
    {
        let descriptor: Py<NativeProperty> = Py::new(
            py,
            NativeProperty {
                property: *property,
                name,
                cached: true,
            },
        )?;
        module.getattr(*class)?.setattr(*name, descriptor)?;
    }
    for (class, name, method) in behaviours().methods.iter().flat_map(|group| group.iter()) {
        let descriptor: Py<NativeMethod> = Py::new(py, NativeMethod { method: *method })?;
        module.getattr(*class)?.setattr(*name, descriptor)?;
    }
    let sequence: Bound<'_, PyAny> = py.import("collections.abc")?.getattr("Sequence")?;
    for class in behaviours().sequences.iter().flat_map(|group| group.iter()) {
        sequence.call_method1("register", (module.getattr(*class)?,))?;
    }
    Ok(())
}

/// A subclass of `parent` laid out as `spec`.
fn error_class<'py>(
    parent: &Bound<'py, PyAny>,
    (index, spec): (usize, &ErrorSpec),
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = parent.py();
    let namespace: Bound<'py, PyDict> = PyDict::new(py);
    namespace.set_item("__module__", NATIVE_MODULE)?;
    namespace.set_item("__qualname__", spec.name)?;
    namespace.set_item("__doc__", spec.doc)?;
    namespace.set_item(ERROR_SPEC_ATTRIBUTE, index)?;
    for (name, value) in spec.class_attributes {
        namespace.set_item(*name, class_value(py, *value)?)?;
    }
    if matches!(spec.parent, EXCEPTION_PARENT | VALUE_ERROR_PARENT) {
        namespace.set_item("__init__", Py::new(py, ErrorInitializer {})?)?;
    }
    parent.get_type().call1((spec.name, (parent,), namespace))
}

/// `StrEnum(name, members)` living in `sqlbuild._native`.
fn string_enum<'py>(str_enum: &Bound<'py, PyAny>, spec: &EnumSpec) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = str_enum.py();
    let options: Bound<'py, PyDict> = PyDict::new(py);
    options.set_item("module", NATIVE_MODULE)?;
    options.set_item("qualname", spec.name)?;
    let members: Vec<(&str, &str)> = spec.members.to_vec();
    let enum_class: Bound<'py, PyAny> = str_enum.call((spec.name, members), Some(&options))?;
    if !spec.doc.is_empty() {
        enum_class.setattr("__doc__", spec.doc)?;
    }
    Ok(enum_class)
}

/// The `dataclasses.Field` table that lets `dataclasses.replace` and `fields` read a record.
fn dataclass_fields<'py>(
    dataclasses: &Bound<'py, PyModule>,
    spec: &RecordSpec,
) -> PyResult<Bound<'py, PyDict>> {
    let py: Python<'py> = dataclasses.py();
    let field_kind: Bound<'py, PyAny> = dataclasses.getattr(DATACLASS_FIELD_KIND)?;
    let fields: Bound<'py, PyDict> = PyDict::new(py);
    for field in spec.fields {
        let options: Bound<'py, PyDict> = PyDict::new(py);
        options.set_item("compare", field.compared)?;
        options.set_item("repr", field.shown)?;
        let declared: Bound<'py, PyAny> =
            native_field(dataclasses, "field")?.call((), Some(&options))?;
        declared.setattr("name", field.name)?;
        declared.setattr("type", "object")?;
        declared.setattr("_field_type", &field_kind)?;
        fields.set_item(field.name, declared)?;
    }
    Ok(fields)
}

/// A `NativeRecord` subclass laid out as `spec`.
fn record_class<'py>(
    metaclass: &Bound<'py, PyAny>,
    base: &Bound<'py, PyAny>,
    (index, spec): (usize, &'static RecordSpec),
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = base.py();
    let namespace: Bound<'py, PyDict> = PyDict::new(py);
    namespace.set_item("__module__", NATIVE_MODULE)?;
    namespace.set_item("__qualname__", spec.name)?;
    namespace.set_item("__doc__", spec.doc)?;
    namespace.set_item("__slots__", PyTuple::empty(py))?;
    namespace.set_item(RECORD_SPEC_ATTRIBUTE, index)?;
    let names: Vec<Bound<'py, PyString>> = spec
        .fields
        .iter()
        .map(|field| PyString::new(py, field.name))
        .collect();
    namespace.set_item("__match_args__", PyTuple::new(py, &names)?)?;
    for (position, field) in spec.fields.iter().enumerate() {
        let attribute: Py<RecordAttribute> = Py::new(
            py,
            RecordAttribute {
                index: position,
                name: field.name,
            },
        )?;
        namespace.set_item(field.name, attribute)?;
    }
    metaclass.call1((spec.name, (base,), namespace))
}
