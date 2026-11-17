//! A config record's values, their typed nodes kept on the record so each key converts once.

use crate::model_config::_helpers::config::typed_config::TYPED_VALUES_SLOT;
use crate::model_config::_helpers::config::typed_config::VALUES_FIELD;
use crate::model_config::models::TypedConfigValues;
use crate::model_config::types::ConfigValues;
use pyo3::prelude::{Bound, PyAny, PyResult, Python};
use sqlbuild_python_base::records::main::fields::cached_value::cached_value;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::main::fields::store_cached::store_cached;
use sqlbuild_python_base::records::models::NativeRecord;
use std::sync::Mutex;

/// A config record's values, their typed nodes kept on the record so each key converts once.
pub fn typed_values<'py>(config: &Bound<'py, PyAny>) -> PyResult<ConfigValues<'py>> {
    let py: Python<'py> = config.py();
    let values: Bound<'py, PyAny> = native_field(config, VALUES_FIELD)?;
    let fresh = || {
        Bound::new(
            py,
            TypedConfigValues {
                built: None,
                read: Mutex::new(Vec::new()),
            },
        )
    };
    let Ok(record) = config.cast::<NativeRecord>() else {
        let typed: Bound<'py, TypedConfigValues> = fresh()?;
        return Ok((values, typed));
    };
    let typed: Bound<'py, TypedConfigValues> = match cached_value(record, TYPED_VALUES_SLOT)? {
        Some(typed) => typed.into_bound(py).cast_into()?,
        None => {
            let typed: Bound<'py, TypedConfigValues> = fresh()?;
            store_cached(
                record,
                TYPED_VALUES_SLOT,
                &typed.clone().into_any().unbind(),
            )?;
            typed
        }
    };
    Ok((values, typed))
}
