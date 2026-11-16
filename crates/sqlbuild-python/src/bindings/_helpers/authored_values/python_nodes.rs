//! Python-node authoring checks: retry policies, audit cases and hook reads, in Python's operators.

use pyo3::exceptions::{PyBaseException, PyTypeError};
use pyo3::prelude::{
    Bound, PyAny, PyAnyMethods, PyErr, PyModule, PyModuleMethods, PyResult, Python,
};
use pyo3::types::{
    PyDict, PyDictMethods, PyInt, PyMemoryView, PyString, PyStringMethods, PyTuple, PyType,
};
use pyo3::types::{PyTupleMethods, PyTypeMethods};
use pyo3::{pyfunction, wrap_pyfunction};
use sqlbuild_core::identifier_text::main::is_lowercase_identifier::is_lowercase_identifier;
use sqlbuild_host::entries::models::{FacadeHost, UserCodeHost};
use sqlbuild_model_config::header_metadata::main::json_meta_problem::json_meta_problem;
use sqlbuild_python_base::native_host::main::host_error::host_error;

use crate::bindings::_helpers::model_config::authored_nodes::PyNode;
use sqlbuild_python_base::boundary::main::imported::imported;
use sqlbuild_python_base::boundary::models::Request;
use sqlbuild_python_base::native_host::main::host_entry::host_entry;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

const RANDOM_MODULE: &str = "random";
const MAX_ATTEMPTS_TOO_LOW: &str = "RetryPolicy max_attempts must be at least 1";
const INITIAL_DELAY_NEGATIVE: &str = "RetryPolicy initial_delay_seconds must be non-negative";
const BACKOFF_TOO_LOW: &str = "RetryPolicy backoff_multiplier must be at least 1";
const MAX_DELAY_NEGATIVE: &str = "RetryPolicy max_delay_seconds must be non-negative";
const MAX_ELAPSED_NEGATIVE: &str = "RetryPolicy max_elapsed_seconds must be non-negative";
const RETRY_ON_NOT_EXCEPTIONS: &str = "RetryPolicy retry_on must contain exception classes";
const AUDIT_CASE_NAME_INVALID: &str = "AuditCase name must be a non-empty lowercase identifier \
                                       using letters, numbers, and underscores";
const AUDIT_CASE_DEFINITION_EMPTY: &str = "AuditCase definition must be non-empty";
const AUDIT_CASE_MINIMUM_SAMPLES: &str = "AuditCase minimum_samples must be a non-negative integer";
const AUDIT_CASE_EVIDENCE_LIMIT: &str = "AuditCase evidence_limit must be a non-negative integer";
const BYTES_NAME_ERROR: &str = "cannot use a string pattern on a bytes-like object";
const HOOK_READS_INVALID: &str =
    "@hook(reads=...) accepts model(...), source(...), and seed(...) references only; got ";

/// `SharedInputError(message)`.
fn shared_input_error(py: Python<'_>, message: &str) -> PyErr {
    host_error(py, FacadeHost::SharedInputError, message)
}

/// `value < bound` under Python's comparison.
fn below(value: &Bound<'_, PyAny>, bound: i64) -> PyResult<bool> {
    value.lt(bound)
}

/// Whether `value` is an exception class.
fn exception_class(value: &Bound<'_, PyAny>) -> PyResult<bool> {
    match value.downcast::<PyType>() {
        Ok(class) => class.is_subclass_of::<PyBaseException>(),
        Err(_) => Ok(false),
    }
}

/// The validated `retry_on` exception classes of a retry policy.
#[pyfunction]
#[pyo3(signature = (**request))]
fn retry_policy_exceptions<'py>(
    request: Option<&Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyTuple>> {
    let request: Request<'_, 'py> = Request::new("retry_policy_exceptions", request)?;
    let py: Python<'py> = request.py();
    let max_delay_seconds: Bound<'py, PyAny> = request.get("max_delay_seconds")?;
    let max_elapsed_seconds: Bound<'py, PyAny> = request.get("max_elapsed_seconds")?;
    if below(&request.get("max_attempts")?, 1)? {
        return Err(shared_input_error(py, MAX_ATTEMPTS_TOO_LOW));
    }
    if below(&request.get("initial_delay_seconds")?, 0)? {
        return Err(shared_input_error(py, INITIAL_DELAY_NEGATIVE));
    }
    if below(&request.get("backoff_multiplier")?, 1)? {
        return Err(shared_input_error(py, BACKOFF_TOO_LOW));
    }
    if !max_delay_seconds.is_none() && below(&max_delay_seconds, 0)? {
        return Err(shared_input_error(py, MAX_DELAY_NEGATIVE));
    }
    if !max_elapsed_seconds.is_none() && below(&max_elapsed_seconds, 0)? {
        return Err(shared_input_error(py, MAX_ELAPSED_NEGATIVE));
    }
    let retry_on: Bound<'py, PyAny> = request.get("retry_on")?;
    if exception_class(&retry_on)? {
        return PyTuple::new(py, [retry_on]);
    }
    let classes: Bound<'py, PyTuple> = PyTuple::new(
        py,
        retry_on
            .try_iter()?
            .collect::<PyResult<Vec<Bound<'py, PyAny>>>>()?,
    )?;
    for class in classes.iter() {
        if !exception_class(&class)? {
            return Err(shared_input_error(py, RETRY_ON_NOT_EXCEPTIONS));
        }
    }
    Ok(classes)
}

/// The delay before retry `retry_index`: exponential backoff, capped, and jittered by `random`.
#[pyfunction]
#[pyo3(signature = (*, retry_policy, retry_index))]
fn retry_delay_seconds<'py>(
    retry_policy: &Bound<'py, PyAny>,
    retry_index: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let initial: Bound<'py, PyAny> = native_field(retry_policy, "initial_delay_seconds")?;
    let growth: Bound<'py, PyAny> = native_field(retry_policy, "backoff_multiplier")?
        .pow(retry_index, retry_policy.py().None())?;
    let mut delay: Bound<'py, PyAny> = initial.mul(growth)?;
    let maximum: Bound<'py, PyAny> = native_field(retry_policy, "max_delay_seconds")?;
    if !maximum.is_none() && maximum.lt(&delay)? {
        delay = maximum;
    }
    if native_field(retry_policy, "jitter")?.is_truthy()? {
        return imported(retry_policy.py(), RANDOM_MODULE, "uniform")?.call1((0, delay));
    }
    Ok(delay)
}

/// The `re.fullmatch` failure for a name that is not a string.
fn name_type_error(name: &Bound<'_, PyAny>) -> PyResult<PyErr> {
    match PyMemoryView::from(name) {
        Ok(_) => return Ok(PyTypeError::new_err(BYTES_NAME_ERROR)),
        Err(error) if error.is_instance_of::<PyTypeError>(name.py()) => {}
        Err(error) => return Err(error),
    }
    Ok(PyTypeError::new_err(format!(
        "expected string or bytes-like object, got '{}'",
        name.get_type().name()?
    )))
}

/// Whether an optional count is absent or a non-negative integer other than a boolean.
fn valid_count(value: &Bound<'_, PyAny>) -> PyResult<bool> {
    if value.is_none() {
        return Ok(true);
    }
    if !value.is_instance_of::<PyInt>() || value.is_instance_of::<pyo3::types::PyBool>() {
        return Ok(false);
    }
    Ok(!below(value, 0)?)
}

/// Reject an audit case without a lowercase name, a definition, valid counts, or JSON meta.
#[pyfunction]
#[pyo3(signature = (**request))]
fn validate_audit_case(request: Option<&Bound<'_, PyDict>>) -> Result<(), PyErr> {
    let request = Request::new("validate_audit_case", request)?;
    let py: Python<'_> = request.py();
    let name: &Bound<'_, PyAny> = &request.get("name")?;
    let definition: &Bound<'_, PyAny> = &request.get("definition")?;
    let minimum_samples: &Bound<'_, PyAny> = &request.get("minimum_samples")?;
    let evidence_limit: &Bound<'_, PyAny> = &request.get("evidence_limit")?;
    let meta: Option<Bound<'_, PyAny>> = request.optional("meta")?;
    let Ok(text) = name.downcast::<PyString>() else {
        return Err(name_type_error(name)?);
    };
    if !is_lowercase_identifier(&text.to_string_lossy()) {
        return Err(shared_input_error(py, AUDIT_CASE_NAME_INVALID));
    }
    if !definition.call_method0("strip")?.is_truthy()? {
        return Err(shared_input_error(py, AUDIT_CASE_DEFINITION_EMPTY));
    }
    if !valid_count(minimum_samples)? {
        return Err(shared_input_error(py, AUDIT_CASE_MINIMUM_SAMPLES));
    }
    if !valid_count(evidence_limit)? {
        return Err(shared_input_error(py, AUDIT_CASE_EVIDENCE_LIMIT));
    }
    if let Some(meta) = meta.as_ref().filter(|meta| !meta.is_none())
        && let Some(problem) = json_meta_problem(&PyNode(meta.clone()))
    {
        let message: String = format!("AuditCase '{text}' meta {problem}");
        return Err(shared_input_error(py, &message));
    }
    Ok(())
}

/// `@hook(reads=...)` references in first-seen order; a leading invalid `None` is tolerated.
#[pyfunction]
fn normalize_hook_reads<'py>(value: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyTuple>> {
    let py: Python<'py> = value.py();
    let reference: Bound<'py, PyAny> = host_entry(py, UserCodeHost::SqlResourceRef)?;
    let reads: Vec<Bound<'py, PyAny>> = if value.is_instance(&reference)? {
        vec![value.clone()]
    } else {
        value.try_iter()?.collect::<PyResult<_>>()?
    };
    for item in &reads {
        if !item.is_instance(&reference)? {
            if item.is_none() {
                break;
            }
            let message: String = format!("{HOOK_READS_INVALID}{}", item.get_type().name()?);
            return Err(shared_input_error(py, &message));
        }
    }
    let distinct: Bound<'py, PyDict> = PyDict::new(py);
    let mut ordered: Vec<Bound<'py, PyAny>> = Vec::new();
    for item in reads {
        if !item.is_instance(&reference)? {
            continue;
        }
        if !distinct.contains(&item)? {
            distinct.set_item(&item, py.None())?;
            ordered.push(item);
        }
    }
    PyTuple::new(py, ordered)
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(retry_policy_exceptions, module)?)?;
    module.add_function(wrap_pyfunction!(retry_delay_seconds, module)?)?;
    module.add_function(wrap_pyfunction!(validate_audit_case, module)?)?;
    module.add_function(wrap_pyfunction!(normalize_hook_reads, module)?)?;
    Ok(())
}
