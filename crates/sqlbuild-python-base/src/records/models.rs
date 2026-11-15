//! Native frozen value records and the field layouts they are built from.

use std::sync::Mutex;

use pyo3::exceptions::PyAttributeError;
use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::pyclass::CompareOp;
use pyo3::types::{PyDict, PyTuple, PyType};
use pyo3::{IntoPyObjectExt, pyclass, pymethods};
use sqlbuild_host::entries::models::HostId;

use crate::records::_helpers::native_classes::fields::{
    build_values, cached_value, compared_values, frozen_error, ordered_comparison, record_spec,
    repr_text, store_cached,
};
use crate::records::types::RecordId;
use crate::records::types::{
    ClassMethod, ClassProperty, GetterClass, GetterRead, RecordFactory, RecordFinish,
};

/// How a record field is filled when the caller does not pass it.
#[derive(Debug, Clone, Copy)]
pub enum FieldDefault {
    Required,
    None,
    False,
    True,
    Int(i64),
    Text(&'static str),
    /// A tuple of texts.
    TextTuple(&'static [&'static str]),
    /// A frozenset of texts.
    TextFrozenset(&'static [&'static str]),
    EmptyTuple,
    EmptyDict,
    EmptyFrozenset,
    /// `getattr(getattr(import(module), class), member)`.
    Member(&'static str, &'static str, &'static str),
    /// A member of a native string enum.
    NativeMember(RecordId, &'static str),
    /// A member of a staying-Python enum reached through the host.
    HostMember(HostId, &'static str),
    /// A native record class called with no arguments, evaluated for every instance.
    NativeCall(RecordId),
    /// A native factory, evaluated for every instance.
    Factory(RecordFactory),
    /// `getattr(import(module), name)(text)`, evaluated for every instance.
    CallText(&'static str, &'static str, &'static str),
}

/// One record field: its name, default, and whether it takes part in equality and in repr.
#[derive(Debug, Clone, Copy)]
pub struct FieldSpec {
    pub name: &'static str,
    pub default: FieldDefault,
    pub compared: bool,
    pub shown: bool,
}

/// The layout of one native record class.
#[derive(Debug, Clone, Copy)]
pub struct RecordSpec {
    pub name: &'static str,
    pub doc: &'static str,
    pub fields: &'static [FieldSpec],
}

/// The members of one native string enum, in declaration order.
#[derive(Debug, Clone, Copy)]
pub struct EnumSpec {
    pub name: &'static str,
    pub doc: &'static str,
    pub members: &'static [(&'static str, &'static str)],
}

/// A class-level constant of a native error class.
#[derive(Debug, Clone, Copy)]
pub enum ClassValue {
    Text(&'static str),
    Int(i64),
}

/// One native expected-error class: its parent, class constants and keyword fields.
#[derive(Debug, Clone, Copy)]
pub struct ErrorSpec {
    pub name: &'static str,
    /// A native error base or an earlier error in `ERRORS`.
    pub parent: &'static str,
    pub doc: &'static str,
    pub class_attributes: &'static [(&'static str, ClassValue)],
    /// Keyword-only constructor fields stored as attributes.
    pub required: &'static [&'static str],
    /// Keyword-only constructor fields stored as attributes, `None` when not passed.
    pub optional: &'static [&'static str],
    /// Attributes every instance is given.
    pub fixed: &'static [(&'static str, ClassValue)],
}

/// Base of every native frozen record, built, compared, shown and pickled as a dataclass.
#[derive(Debug)]
#[pyclass(module = "sqlbuild._native", subclass, frozen)]
pub struct NativeRecord {
    /// The layout of the record's class.
    pub spec: &'static RecordSpec,
    pub values: Vec<Py<PyAny>>,
    /// Values of the record's cached computed attributes, by attribute name.
    pub cached: Mutex<Vec<(&'static str, Py<PyAny>)>>,
}

#[pymethods]
impl NativeRecord {
    #[new]
    #[classmethod]
    #[pyo3(signature = (*args, **kwargs))]
    fn new(
        cls: &Bound<'_, PyType>,
        args: &Bound<'_, PyTuple>,
        kwargs: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Self> {
        let spec: &RecordSpec = record_spec(cls)?;
        Ok(Self {
            spec,
            values: build_values(spec, args, kwargs)?,
            cached: Mutex::new(Vec::new()),
        })
    }

    fn __eq__(slf: &Bound<'_, Self>, other: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        let py: Python<'_> = slf.py();
        if !other.get_type().is(slf.get_type()) {
            return Ok(py.NotImplemented());
        }
        let other: &Bound<'_, Self> = other.cast::<Self>()?;
        compared_values(slf)?
            .as_any()
            .eq(compared_values(other)?)?
            .into_py_any(py)
    }

    fn __lt__(slf: &Bound<'_, Self>, other: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        ordered_comparison(slf, other, CompareOp::Lt)
    }

    fn __le__(slf: &Bound<'_, Self>, other: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        ordered_comparison(slf, other, CompareOp::Le)
    }

    fn __gt__(slf: &Bound<'_, Self>, other: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        ordered_comparison(slf, other, CompareOp::Gt)
    }

    fn __ge__(slf: &Bound<'_, Self>, other: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        ordered_comparison(slf, other, CompareOp::Ge)
    }

    fn __hash__(slf: &Bound<'_, Self>) -> PyResult<isize> {
        compared_values(slf)?.as_any().hash()
    }

    fn __repr__(slf: &Bound<'_, Self>) -> PyResult<String> {
        repr_text(slf)
    }

    fn __reduce__<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyTuple>> {
        let py: Python<'py> = slf.py();
        let values: Bound<'py, PyTuple> = PyTuple::new(py, slf.get().values.iter())?;
        PyTuple::new(py, [slf.get_type().into_any(), values.into_any()])
    }
}

/// A computed attribute of a native class, evaluated by its native function.
#[derive(Debug)]
#[pyclass(module = "sqlbuild._native", frozen)]
pub struct NativeProperty {
    pub property: ClassProperty,
    pub name: &'static str,
    /// Whether a record computes the attribute once and keeps it.
    pub cached: bool,
}

#[pymethods]
impl NativeProperty {
    fn __get__(
        slf: &Bound<'_, Self>,
        instance: &Bound<'_, PyAny>,
        _owner: &Bound<'_, PyAny>,
    ) -> PyResult<Py<PyAny>> {
        if instance.is_none() {
            return Ok(slf.clone().into_any().unbind());
        }
        let descriptor: &Self = slf.get();
        if !descriptor.cached || !instance.is_instance_of::<NativeRecord>() {
            return (descriptor.property)(instance);
        }
        let record: &Bound<'_, NativeRecord> = instance.cast::<NativeRecord>()?;
        if let Some(value) = cached_value(record, descriptor.name)? {
            return Ok(value);
        }
        let value: Py<PyAny> = (descriptor.property)(instance)?;
        store_cached(record, descriptor.name, &value)?;
        Ok(value)
    }

    fn __set__(&self, instance: &Bound<'_, PyAny>, _value: &Bound<'_, PyAny>) -> PyResult<()> {
        Err(frozen_error(instance.py(), self.name))
    }
}

/// A method of a native class, run by its native function.
#[derive(Debug)]
#[pyclass(module = "sqlbuild._native", frozen)]
pub struct NativeMethod {
    pub method: ClassMethod,
}

#[pymethods]
impl NativeMethod {
    fn __get__(
        slf: &Bound<'_, Self>,
        instance: &Bound<'_, PyAny>,
        owner: &Bound<'_, PyAny>,
    ) -> PyResult<Py<PyAny>> {
        let receiver: &Bound<'_, PyAny> = if instance.is_none() { owner } else { instance };
        Ok(Py::new(
            slf.py(),
            BoundNativeMethod {
                method: slf.get().method,
                receiver: receiver.clone().unbind(),
            },
        )?
        .into_any())
    }
}

/// A `NativeMethod` bound to the instance or class it was read from.
#[derive(Debug)]
#[pyclass(module = "sqlbuild._native", frozen)]
pub struct BoundNativeMethod {
    method: ClassMethod,
    receiver: Py<PyAny>,
}

#[pymethods]
impl BoundNativeMethod {
    #[pyo3(signature = (*args, **kwargs))]
    fn __call__(
        &self,
        args: &Bound<'_, PyTuple>,
        kwargs: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Py<PyAny>> {
        (self.method)(self.receiver.bind(args.py()), args, kwargs)
    }
}

/// Read-only access to one field of a native record.
#[derive(Debug)]
#[pyclass(module = "sqlbuild._native", frozen)]
pub struct RecordAttribute {
    pub index: usize,
    pub name: &'static str,
}

#[pymethods]
impl RecordAttribute {
    fn __get__(
        slf: &Bound<'_, Self>,
        instance: &Bound<'_, PyAny>,
        _owner: &Bound<'_, PyAny>,
    ) -> PyResult<Py<PyAny>> {
        let py: Python<'_> = slf.py();
        if instance.is_none() {
            return Ok(slf.clone().into_any().unbind());
        }
        let record: &Bound<'_, NativeRecord> = instance.cast::<NativeRecord>()?;
        let attribute: &Self = slf.get();
        record
            .get()
            .values
            .get(attribute.index)
            .map(|value| value.clone_ref(py))
            .ok_or_else(|| PyAttributeError::new_err(attribute.name))
    }

    fn __set__(&self, _instance: &Bound<'_, PyAny>, _value: &Bound<'_, PyAny>) -> PyResult<()> {
        Err(frozen_error(_instance.py(), self.name))
    }

    fn __delete__(&self, _instance: &Bound<'_, PyAny>) -> PyResult<()> {
        Err(frozen_error(_instance.py(), self.name))
    }
}

impl FieldSpec {
    /// A field every caller must pass.
    pub const fn required(name: &'static str) -> Self {
        Self::new(name, FieldDefault::Required)
    }

    /// A field filled from `default` when it is not passed.
    pub const fn new(name: &'static str, default: FieldDefault) -> Self {
        Self {
            name,
            default,
            compared: true,
            shown: true,
        }
    }

    /// A field left out of equality, hashing and repr.
    pub const fn uncompared(name: &'static str, default: FieldDefault) -> Self {
        Self {
            name,
            default,
            compared: false,
            shown: false,
        }
    }
}

/// One native pyclass and the reader of its `#[getter]` attributes.
#[derive(Clone, Copy, Debug)]
pub struct GetterEntry {
    pub class: GetterClass,
    pub read: GetterRead,
}

/// The record behaviours registration installs, each as groups searched in order.
#[derive(Clone, Copy, Debug)]
pub struct RecordBehaviours {
    /// Records whose bound values are normalized or validated before the record is created.
    pub finishers: &'static [&'static [(&'static str, RecordFinish)]],
    /// `(class, attribute, getter)` computed attributes set on native classes after registration.
    pub properties: &'static [&'static [(&'static str, &'static str, ClassProperty)]],
    /// `(class, attribute, getter)` computed attributes a record computes once and keeps.
    pub cached_properties: &'static [&'static [(&'static str, &'static str, ClassProperty)]],
    /// `(class, attribute, method)` methods set on native classes after registration.
    pub methods: &'static [&'static [(&'static str, &'static str, ClassMethod)]],
    /// Records ordered by their compared fields, as `order=True` dataclasses are.
    pub ordered: &'static [&'static [&'static str]],
    /// Native records registered as `collections.abc.Sequence` virtual subclasses.
    pub sequences: &'static [&'static [&'static str]],
}
