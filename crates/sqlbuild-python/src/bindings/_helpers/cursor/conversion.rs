//! One call's cursor values read from and built as the native cursor value records.

use std::cell::RefCell;

use pyo3::IntoPyObject;
use pyo3::exceptions::{PyOverflowError, PyTypeError, PyValueError};
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyErr, PyResult, Python};
use pyo3::types::PyString;
use pyo3::types::PyStringMethods;
use sqlbuild_cursor::algebra::errors::CursorError;
use sqlbuild_cursor::algebra::models::{
    Grain, Interval, LocalDateTime, Scalar, ScalarValue, Timestamp, ZoneOffset,
};
use sqlbuild_cursor::algebra::types::ZoneRules;
use sqlbuild_host::entries::models::FacadeHost;

use crate::bindings::_helpers::boundary::python_calls::{call_with, imported};
use crate::bindings::_helpers::cursor::values::{
    DATETIME_MODULE, civil_date, local_date_time, naive_date_time, plain_date, replaced,
    timedelta_micros,
};
use crate::bindings::_helpers::native_host::host;
use crate::bindings::_helpers::records::construct::{
    enum_from_value, new_record_with, record_class,
};
use crate::bindings::_helpers::records::fields::field as native_field;
use crate::bindings::_helpers::records::ids::RecordId;

/// Cursor grain names, finest first.
pub(crate) const GRAIN_NAMES: [(&str, Grain); 6] = [
    ("second", Grain::Second),
    ("minute", Grain::Minute),
    ("hour", Grain::Hour),
    ("day", Grain::Day),
    ("month", Grain::Month),
    ("year", Grain::Year),
];

/// `CursorAlgebraError(message)`.
pub(crate) fn algebra_error(py: Python<'_>, message: &str) -> PyErr {
    match host::entry(py, FacadeHost::CursorAlgebraError).and_then(|class| class.call1((message,)))
    {
        Ok(error) => PyErr::from_value(error),
        Err(error) => error,
    }
}

/// The cursor values one native call reads, so unchanged inputs come back as the same objects.
pub(crate) struct CursorSession<'py> {
    py: Python<'py>,
    timestamp_class: Bound<'py, PyAny>,
    date_class: Bound<'py, PyAny>,
    integer_class: Bound<'py, PyAny>,
    /// Each input value object and the `date`, `datetime` or `int` it holds.
    inputs: RefCell<Vec<(Bound<'py, PyAny>, Bound<'py, PyAny>)>>,
    /// The Python error a zone lookup raised, re-raised in place of the algebra's error.
    failure: RefCell<Option<PyErr>>,
}

impl<'py> CursorSession<'py> {
    pub(crate) fn new(py: Python<'py>) -> PyResult<Self> {
        Ok(Self {
            py,
            timestamp_class: record_class(py, RecordId::TimestampValue)?,
            date_class: record_class(py, RecordId::DateValue)?,
            integer_class: record_class(py, RecordId::IntegerValue)?,
            inputs: RefCell::new(Vec::new()),
            failure: RefCell::new(None),
        })
    }

    pub(crate) fn py(&self) -> Python<'py> {
        self.py
    }

    /// Whether `value` is one of the three typed cursor values.
    pub(crate) fn scalar_typed(&self, value: &Bound<'py, PyAny>) -> PyResult<bool> {
        Ok(value.is_instance(&self.timestamp_class)?
            || value.is_instance(&self.date_class)?
            || value.is_instance(&self.integer_class)?)
    }

    fn timestamp(&self, inner: &Bound<'py, PyAny>, handle: usize) -> PyResult<Timestamp> {
        let tzinfo: Bound<'py, PyAny> = native_field(inner, "tzinfo")?;
        let offset: ZoneOffset = if tzinfo.is_none() {
            ZoneOffset::Naive
        } else if tzinfo
            .get_type()
            .is(&imported(self.py, DATETIME_MODULE, "timezone")?)
        {
            ZoneOffset::Fixed(timedelta_micros(
                &tzinfo.call_method1("utcoffset", (self.py.None(),))?,
            )?)
        } else {
            ZoneOffset::External(handle)
        };
        Ok(Timestamp {
            local: local_date_time(inner)?,
            fold: native_field(inner, "fold")?.extract::<u8>()? != 0,
            offset,
            template: Some(handle),
            source_text: None,
        })
    }

    /// A typed cursor value object as a scalar remembering the object.
    pub(crate) fn scalar(&self, value: &Bound<'py, PyAny>) -> PyResult<Scalar> {
        let inner: Bound<'py, PyAny> = native_field(value, "value")?;
        let handle: usize = self.inputs.borrow().len();
        let typed: ScalarValue = if value.is_instance(&self.integer_class)? {
            ScalarValue::Integer(inner.extract()?)
        } else if value.is_instance(&self.date_class)? {
            ScalarValue::Date(civil_date(&inner)?)
        } else {
            let mut timestamp: Timestamp = self.timestamp(&inner, handle)?;
            let source_text: Bound<'py, PyAny> = native_field(value, "source_text")?;
            timestamp.source_text = (!source_text.is_none())
                .then(|| source_text.extract())
                .transpose()?;
            ScalarValue::Timestamp(timestamp)
        };
        self.inputs.borrow_mut().push((value.clone(), inner));
        Ok(Scalar {
            value: typed,
            handle: Some(handle),
        })
    }

    /// `CursorGrain | None` as a grain; an unknown name fails as `CursorGrain(name)` does.
    pub(crate) fn grain(&self, grain: &Bound<'py, PyAny>) -> PyResult<Option<Grain>> {
        if grain.is_none() {
            return Ok(None);
        }
        if let Ok(name) = grain.downcast::<PyString>() {
            let name: String = name.to_str()?.to_owned();
            if let Some((_, known)) = GRAIN_NAMES.iter().find(|(known, _)| *known == name) {
                return Ok(Some(*known));
            }
        }
        enum_from_value(self.py, RecordId::CursorGrain, grain)?;
        Err(PyTypeError::new_err("cursor grain must be a CursorGrain"))
    }

    /// An `AlignedInterval` as an interval.
    pub(crate) fn interval(&self, interval: &Bound<'py, PyAny>) -> PyResult<Interval> {
        Ok(Interval {
            start: self.scalar(&native_field(interval, "start")?)?,
            end: self.scalar(&native_field(interval, "end")?)?,
            grain: self.grain(&native_field(interval, "grain")?)?,
        })
    }

    fn input(&self, handle: usize, inner: bool) -> Bound<'py, PyAny> {
        let inputs = self.inputs.borrow();
        let (value, held) = &inputs[handle];
        if inner { held.clone() } else { value.clone() }
    }

    /// The `datetime` holding `local` in the zone and type of input `template`.
    fn date_time(&self, timestamp: &Timestamp) -> PyResult<Bound<'py, PyAny>> {
        match timestamp.template {
            Some(template) => replaced(
                &self.input(template, true),
                &timestamp.local,
                timestamp.fold,
            ),
            None => naive_date_time(self.py, &timestamp.local, timestamp.fold),
        }
    }

    /// The input object for an unchanged value, else a new typed cursor value.
    pub(crate) fn python_scalar(&self, scalar: &Scalar) -> PyResult<Bound<'py, PyAny>> {
        if let Some(handle) = scalar.handle {
            return Ok(self.input(handle, false));
        }
        let (class, value): (&Bound<'py, PyAny>, Bound<'py, PyAny>) = match &scalar.value {
            ScalarValue::Integer(number) => (
                &self.integer_class,
                number.into_pyobject(self.py)?.into_any(),
            ),
            ScalarValue::Date(date) => (&self.date_class, plain_date(self.py, date)?),
            ScalarValue::Timestamp(timestamp) => {
                (&self.timestamp_class, self.date_time(timestamp)?)
            }
        };
        call_with(class, &[("value", value)])
    }

    /// A grain as its `CursorGrain` member.
    pub(crate) fn python_grain(&self, grain: Option<Grain>) -> PyResult<Bound<'py, PyAny>> {
        let Some(grain) = grain else {
            return Ok(self.py.None().into_bound(self.py));
        };
        let name: &str = GRAIN_NAMES
            .iter()
            .find(|(_, known)| *known == grain)
            .map_or("", |(name, _)| name);
        enum_from_value(self.py, RecordId::CursorGrain, name)
    }

    /// A new `AlignedInterval`, validated as its constructor validates.
    pub(crate) fn python_interval(&self, interval: &Interval) -> PyResult<Bound<'py, PyAny>> {
        new_record_with(
            self.py,
            RecordId::AlignedInterval,
            &[
                ("start", self.python_scalar(&interval.start)?),
                ("end", self.python_scalar(&interval.end)?),
                ("grain", self.python_grain(interval.grain)?),
            ],
        )
    }

    /// The Python exception for a failed cursor operation.
    pub(crate) fn error(&self, error: CursorError) -> PyErr {
        if let Some(failure) = self.failure.borrow_mut().take() {
            return failure;
        }
        match error {
            CursorError::Algebra(message) => algebra_error(self.py, &message),
            CursorError::Overflow(message) => PyOverflowError::new_err(message),
            CursorError::Value(message) => PyValueError::new_err(message),
            CursorError::Type(message) => PyTypeError::new_err(message),
        }
    }

    /// `result`, with a cursor error raised as its Python exception.
    pub(crate) fn checked<T>(&self, result: Result<T, CursorError>) -> PyResult<T> {
        result.map_err(|error| self.error(error))
    }

    fn zone_call(
        &self,
        template: usize,
        local: &LocalDateTime,
        fold: bool,
    ) -> PyResult<Bound<'py, PyAny>> {
        replaced(&self.input(template, true), local, fold)
    }

    fn remember(&self, error: PyErr) -> CursorError {
        let message: String = error.to_string();
        *self.failure.borrow_mut() = Some(error);
        CursorError::Algebra(message)
    }
}

impl ZoneRules for CursorSession<'_> {
    fn to_utc(
        &self,
        template: usize,
        local: &LocalDateTime,
        fold: bool,
    ) -> Result<LocalDateTime, CursorError> {
        let converted = self.zone_call(template, local, fold).and_then(|value| {
            let utc: Bound<'_, PyAny> = imported(self.py, DATETIME_MODULE, "UTC")?;
            local_date_time(&value.call_method1("astimezone", (utc,))?)
        });
        converted.map_err(|error| self.remember(error))
    }

    fn utc_offset(
        &self,
        template: usize,
        local: &LocalDateTime,
        fold: bool,
    ) -> Result<Option<i64>, CursorError> {
        let offset = self.zone_call(template, local, fold).and_then(|value| {
            let delta: Bound<'_, PyAny> = value.call_method0("utcoffset")?;
            if delta.is_none() {
                return Ok(None);
            }
            timedelta_micros(&delta).map(Some)
        });
        offset.map_err(|error| self.remember(error))
    }
}
