//! Project-local adapter nullability rules, run by calling the adapter's own Python rule.

use std::sync::{Arc, Mutex, PoisonError};

use pyo3::prelude::{Py, PyAny, PyAnyMethods, PyDictMethods, PyErr, Python};
use pyo3::types::{PyDict, PyTuple};
use sqlbuild_analysis::assembly::analysis_session::models::NullabilityCallback;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// The first exception an adapter rule raised, which the session re-raises to Python.
pub(crate) type RuleFailure = Arc<Mutex<Option<PyErr>>>;

const NULLABILITIES: [&str; 3] = ["non_null", "nullable", "unknown"];
const RULE_RAISED: &str = "an adapter nullability rule raised";

/// The callback calling `rules[name](tuple(InferredNullability(value) for value in args))`.
pub(crate) fn nullability_callback(
    rules: Py<PyDict>,
    nullability: Py<PyAny>,
    failure: RuleFailure,
) -> NullabilityCallback {
    NullabilityCallback(Arc::new(move |name: &str, arguments: &[&'static str]| {
        Python::attach(|python| {
            let called = || -> Result<&'static str, PyErr> {
                let enum_type = nullability.bind(python);
                let rule = rules
                    .bind(python)
                    .get_item(name)?
                    .ok_or_else(|| pyo3::exceptions::PyKeyError::new_err(name.to_owned()))?;
                let values = arguments
                    .iter()
                    .map(|value| enum_type.call1((*value,)))
                    .collect::<Result<Vec<_>, _>>()?;
                let result = rule.call1((PyTuple::new(python, values)?,))?;
                let value: String =
                    native_field(&enum_type.call1((result,))?, "value")?.extract()?;
                Ok(NULLABILITIES
                    .into_iter()
                    .find(|known| *known == value)
                    .unwrap_or("unknown"))
            };
            called().map_err(|error| {
                let mut slot = failure.lock().unwrap_or_else(PoisonError::into_inner);
                if slot.is_none() {
                    *slot = Some(error);
                }
                RULE_RAISED.to_owned()
            })
        })
    }))
}

/// Take the exception an adapter rule raised, if any.
pub(crate) fn raised(failure: &RuleFailure) -> Option<PyErr> {
    failure
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .take()
}
