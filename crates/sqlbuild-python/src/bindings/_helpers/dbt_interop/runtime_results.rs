//! Backfill streamed dbt results using Python's JSON and UTF-8 artifact contract.

use pyo3::exceptions::PyOSError;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use pyo3::types::{
    PyDict, PyDictMethods, PyFloat, PyInt, PyList, PyListMethods, PyString, PyTuple,
};
use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::RecordId;
use std::collections::HashSet;

pub(crate) fn result_ids(results: &Bound<'_, PyAny>) -> PyResult<HashSet<String>> {
    results
        .try_iter()?
        .map(|result| native_field(&result?, "unique_id")?.extract())
        .collect()
}

fn messages<'py>(raw: &Bound<'py, PyDict>) -> PyResult<Bound<'py, PyTuple>> {
    let py = raw.py();
    let text = if let Some(message) = raw
        .get_item("message")?
        .filter(|value| value.is_instance_of::<PyString>())
    {
        let stripped = message.call_method0("strip")?;
        stripped.is_truthy()?.then_some(stripped)
    } else {
        None
    };
    let text = if text.is_some() {
        text
    } else if let Some(failures) = raw
        .get_item("failures")?
        .filter(|value| value.is_instance_of::<PyInt>())
    {
        if failures.gt(0)? {
            Some(PyString::new(py, &format!("{} failure(s)", failures.str()?)).into_any())
        } else {
            None
        }
    } else {
        None
    };
    let Some(text) = text else {
        return PyTuple::new(py, Vec::<Bound<'py, PyAny>>::new());
    };
    PyTuple::new(
        py,
        [new_record_with(
            py,
            RecordId::DbtNodeMessage,
            &[
                ("level", PyString::new(py, "error").into_any()),
                ("message", text),
            ],
        )?],
    )
}

pub(crate) fn append_missing<'py>(
    results: &Bound<'py, PyAny>,
    target: &Bound<'py, PyAny>,
    expected: Option<&HashSet<String>>,
) -> PyResult<Bound<'py, PyAny>> {
    let py = results.py();
    if target.is_none() {
        return Ok(results.clone());
    }
    let path = target.call_method1("__truediv__", ("run_results.json",))?;
    if !path.call_method0("exists")?.is_truthy()? {
        return Ok(results.clone());
    }
    let existing = result_ids(results)?;
    let json = py.import("json")?;
    let payload = (|| {
        let text = call_with(
            &native_field(&path, "read_text")?,
            &[("encoding", PyString::new(py, "utf-8").into_any())],
        )?;
        native_field(&json, "loads")?.call1((text,))
    })();
    let payload = match payload {
        Ok(payload) => payload,
        Err(error)
            if error.is_instance_of::<PyOSError>(py)
                || error.is_instance(py, &native_field(&json, "JSONDecodeError")?) =>
        {
            return Ok(results.clone());
        }
        Err(error) => return Err(error),
    };
    let Ok(payload) = payload.cast::<PyDict>() else {
        return Ok(results.clone());
    };
    let Some(raw_results) = payload.get_item("results")? else {
        return Ok(results.clone());
    };
    let Ok(raw_results) = raw_results.cast::<PyList>() else {
        return Ok(results.clone());
    };
    let mut appended = results.try_iter()?.collect::<PyResult<Vec<_>>>()?;
    for (index, raw) in raw_results.iter().enumerate() {
        let Ok(raw) = raw.cast::<PyDict>() else {
            continue;
        };
        let Some(unique_id) = raw
            .get_item("unique_id")?
            .filter(|value| value.is_instance_of::<PyString>())
        else {
            continue;
        };
        let id: String = unique_id.extract()?;
        if existing.contains(&id) || expected.is_some_and(|ids| !ids.contains(&id)) {
            continue;
        }
        let resource_type = id.split_once('.').map_or("node", |(kind, _)| kind);
        let name = id.rsplit('.').next().unwrap_or(&id);
        let status = raw
            .get_item("status")?
            .filter(|value| value.is_instance_of::<PyString>())
            .unwrap_or_else(|| PyString::new(py, "unknown").into_any());
        let time = if let Some(value) = raw
            .get_item("execution_time")?
            .filter(|value| value.is_instance_of::<PyInt>() || value.is_instance_of::<PyFloat>())
        {
            py.import("builtins")?.getattr("float")?.call1((value,))?
        } else {
            py.None().into_bound(py)
        };
        appended.push(new_record_with(
            py,
            RecordId::DbtNodeExecutionResult,
            &[
                ("unique_id", unique_id),
                ("resource_type", PyString::new(py, resource_type).into_any()),
                ("node_name", PyString::new(py, name).into_any()),
                ("status", status),
                ("index", PyInt::new(py, index + 1).into_any()),
                ("total", PyInt::new(py, raw_results.len()).into_any()),
                ("execution_time", time),
                ("messages", messages(raw)?.into_any()),
            ],
        )?);
    }
    Ok(PyTuple::new(py, appended)?.into_any())
}
