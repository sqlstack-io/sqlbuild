//! Row-diff tolerance and sampling configuration from model config and CLI overrides.

use pyo3::prelude::{
    Bound, PyAny, PyAnyMethods, PyErr, PyModule, PyModuleMethods, PyResult, Python,
};
use pyo3::types::{PyBool, PyDict, PyDictMethods, PyFloat, PyInt, PyString};
use pyo3::{IntoPyObjectExt, pyfunction, wrap_pyfunction};
use sqlbuild_host::entries::models::{AdapterHost, ExecutorHost};

use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::boundary::models::Request;
use sqlbuild_python_base::native_host::main::host_entry::host_entry;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

const DECIMAL_MODULE: &str = "decimal";
const ABSOLUTE_KEY: &str = "absolute";
const RELATIVE_KEY: &str = "relative";
const TOLERANCE_KEYS: [&str; 2] = [ABSOLUTE_KEY, RELATIVE_KEY];
const DEFAULT_TOLERANCES_LABEL: &str = "row_diff_tolerances";
const CLI_TOLERANCE_MESSAGE: &str =
    "diff --tolerance must use COLUMN:absolute=VALUE or COLUMN:relative=VALUE";

/// `ExecutorInputError(message, code=code)`.
fn input_error(py: Python<'_>, message: String, code: &str) -> PyErr {
    host_entry(py, ExecutorHost::ExecutorInputError)
        .and_then(|class| {
            let keywords: Bound<'_, PyDict> = PyDict::new(py);
            keywords.set_item("code", code)?;
            class.call((message,), Some(&keywords))
        })
        .map_or_else(|error| error, PyErr::from_value)
}

/// `RowDiffTolerances` parsed from raw model configuration.
#[pyfunction]
#[pyo3(signature = (*, raw, label = DEFAULT_TOLERANCES_LABEL))]
fn parse_row_diff_tolerances<'py>(
    raw: Bound<'py, PyAny>,
    label: &str,
) -> PyResult<Bound<'py, PyAny>> {
    parsed_tolerances(&raw, label)
}

fn parsed_tolerances<'py>(raw: &Bound<'py, PyAny>, label: &str) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = raw.py();
    let class: Bound<'py, PyAny> = host_entry(py, AdapterHost::RowDiffTolerances)?;
    if raw.is_none() {
        return class.call0();
    }
    let Ok(mapping) = raw.cast::<PyDict>() else {
        return Err(input_error(
            py,
            format!("{label} must be a mapping"),
            "X401",
        ));
    };
    let by_type: Bound<'py, PyDict> = tolerance_section(
        py,
        mapping.get_item("by_type")?,
        (&format!("{label}.by_type"), true),
    )?;
    let by_column: Bound<'py, PyDict> = tolerance_section(
        py,
        mapping.get_item("by_column")?,
        (&format!("{label}.by_column"), false),
    )?;
    call_with(
        &class,
        &[
            ("by_type", by_type.into_any()),
            ("by_column", by_column.into_any()),
        ],
    )
}

fn tolerance_section<'py>(
    py: Python<'py>,
    raw: Option<Bound<'py, PyAny>>,
    (label, normalize_key): (&str, bool),
) -> PyResult<Bound<'py, PyDict>> {
    let parsed: Bound<'py, PyDict> = PyDict::new(py);
    let Some(raw) = raw.filter(|raw| !raw.is_none()) else {
        return Ok(parsed);
    };
    let Ok(mapping) = raw.cast::<PyDict>() else {
        return Err(input_error(
            py,
            format!("{label} must be a mapping"),
            "X401",
        ));
    };
    for (key, rule) in mapping.iter() {
        let Ok(text) = key.cast::<PyString>() else {
            return Err(input_error(
                py,
                format!("{label} keys must be non-empty strings"),
                "X402",
            ));
        };
        let key_text: String = text.extract()?;
        if key_text.is_empty() {
            return Err(input_error(
                py,
                format!("{label} keys must be non-empty strings"),
                "X402",
            ));
        }
        let parsed_key: Bound<'py, PyAny> = if normalize_key {
            key.call_method0("lower")?
        } else {
            key.clone()
        };
        parsed.set_item(
            parsed_key,
            tolerance_rule(&rule, &format!("{label}.{key_text}"))?,
        )?;
    }
    Ok(parsed)
}

fn tolerance_rule<'py>(raw: &Bound<'py, PyAny>, label: &str) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = raw.py();
    let Ok(mapping) = raw.cast::<PyDict>() else {
        return Err(input_error(
            py,
            format!("{label} must be a mapping"),
            "X401",
        ));
    };
    let mut unsupported: Vec<String> = Vec::new();
    for key in mapping.keys() {
        let known: bool = key
            .extract::<String>()
            .is_ok_and(|text| TOLERANCE_KEYS.contains(&text.as_str()));
        if !known {
            unsupported.push(key.str()?.extract()?);
        }
    }
    if !unsupported.is_empty() {
        return Err(input_error(
            py,
            format!(
                "{label} contains unsupported keys: {}",
                unsupported.join(", ")
            ),
            "X403",
        ));
    }
    let absolute: Option<Bound<'py, PyAny>> = optional_decimal(
        mapping.get_item(ABSOLUTE_KEY)?,
        &format!("{label}.absolute"),
    )?;
    let relative: Option<Bound<'py, PyAny>> = optional_decimal(
        mapping.get_item(RELATIVE_KEY)?,
        &format!("{label}.relative"),
    )?;
    if absolute.is_none() && relative.is_none() {
        return Err(input_error(
            py,
            format!("{label} must define absolute or relative"),
            "X404",
        ));
    }
    call_with(
        &host_entry(py, AdapterHost::RowDiffTolerance)?,
        &[
            (
                "absolute",
                absolute.unwrap_or_else(|| py.None().into_bound(py)),
            ),
            (
                "relative",
                relative.unwrap_or_else(|| py.None().into_bound(py)),
            ),
        ],
    )
}

fn optional_decimal<'py>(
    raw: Option<Bound<'py, PyAny>>,
    label: &str,
) -> PyResult<Option<Bound<'py, PyAny>>> {
    let Some(raw) = raw.filter(|raw| !raw.is_none()) else {
        return Ok(None);
    };
    let py: Python<'py> = raw.py();
    let decimal_module: Bound<'py, PyModule> = py.import(DECIMAL_MODULE)?;
    let decimal_class: Bound<'py, PyAny> = native_field(&decimal_module, "Decimal")?;
    let numeric: bool = !raw.is_instance_of::<PyBool>()
        && (raw.is_instance_of::<PyString>()
            || raw.is_instance_of::<PyInt>()
            || raw.is_instance_of::<PyFloat>()
            || raw.is_instance(&decimal_class)?);
    if !numeric {
        return Err(input_error(py, format!("{label} must be numeric"), "X405"));
    }
    match decimal_class.call1((raw.str()?,)) {
        Ok(value) => Ok(Some(value)),
        Err(error)
            if error.is_instance(py, &native_field(&decimal_module, "InvalidOperation")?) =>
        {
            let replacement: PyErr = input_error(py, format!("{label} must be numeric"), "X405");
            replacement.set_cause(py, Some(error));
            Err(replacement)
        }
        Err(error) => Err(error),
    }
}

fn optional_integer<'py>(raw: &Bound<'py, PyAny>, label: &str) -> PyResult<Option<i64>> {
    if raw.is_none() {
        return Ok(None);
    }
    if raw.is_instance_of::<PyBool>() || !raw.is_instance_of::<PyInt>() {
        return Err(input_error(
            raw.py(),
            format!("{label} must be an integer"),
            "X406",
        ));
    }
    raw.extract().map(Some)
}

/// `RowDiffTolerances` from repeated `COLUMN:KIND=VALUE` CLI specifications.
#[pyfunction]
#[pyo3(signature = (*, values))]
fn parse_cli_tolerance_overrides<'py>(
    py: Python<'py>,
    values: Vec<String>,
) -> PyResult<Bound<'py, PyAny>> {
    cli_tolerance_overrides(py, &values)
}

/// `parse_cli_tolerance_overrides` for native callers.
pub(crate) fn cli_tolerance_overrides<'py>(
    py: Python<'py>,
    values: &[String],
) -> PyResult<Bound<'py, PyAny>> {
    let by_column: Bound<'py, PyDict> = PyDict::new(py);
    for value in values {
        let (column_and_kind, amount) = value.split_once('=').unwrap_or((value, ""));
        let separator: bool = value.contains('=');
        let (column, kind) = column_and_kind
            .split_once(':')
            .map_or((column_and_kind, None), |(column, kind)| {
                (column, Some(kind))
            });
        let valid_kind: Option<&str> = kind.filter(|kind| TOLERANCE_KEYS.contains(kind));
        let (Some(kind), true, false, false) =
            (valid_kind, separator, column.is_empty(), amount.is_empty())
        else {
            return Err(input_error(py, CLI_TOLERANCE_MESSAGE.to_owned(), "X408"));
        };
        let rule: Bound<'py, PyDict> = match by_column.get_item(column)? {
            Some(rule) => rule.cast_into()?,
            None => {
                let rule: Bound<'py, PyDict> = PyDict::new(py);
                by_column.set_item(column, &rule)?;
                rule
            }
        };
        rule.set_item(kind, amount)?;
    }
    let raw: Bound<'py, PyDict> = PyDict::new(py);
    raw.set_item("by_column", by_column)?;
    parsed_tolerances(raw.as_any(), DEFAULT_TOLERANCES_LABEL)
}

/// Model sampling configuration resolved with invocation overrides, or `None`.
#[pyfunction]
#[pyo3(signature = (**options))]
fn resolve_row_diff_sampling<'py>(
    options: Option<&Bound<'py, PyDict>>,
) -> PyResult<Option<Bound<'py, PyAny>>> {
    let request = Request::new("resolve_row_diff_sampling", options)?;
    let py: Python<'py> = request.py();
    let override_values: Bound<'py, PyAny> = request.get("override")?;
    if native_field(&override_values, "exhaustive")?.is_truthy()? {
        return Ok(None);
    }
    let label: String = request.get("label")?.extract()?;
    let overridden = |field: &str, raw: &str| -> PyResult<Bound<'py, PyAny>> {
        let value: Bound<'py, PyAny> = native_field(&override_values, field)?;
        if value.is_none() {
            request.get(raw)
        } else {
            Ok(value)
        }
    };
    let row_limit_label: String = format!("{label}.row_diff_sample_rows");
    let row_limit: Option<i64> =
        optional_integer(&overridden("row_limit", "raw_row_limit")?, &row_limit_label)?;
    if row_limit.is_some_and(|value| value < 0) {
        return Err(input_error(
            py,
            format!("{row_limit_label} must be zero or greater"),
            "X407",
        ));
    }
    let Some(row_limit) = row_limit.filter(|value| *value != 0) else {
        return Ok(None);
    };
    let seed: Option<i64> = optional_integer(
        &overridden("seed", "raw_seed")?,
        &format!("{label}.row_diff_sample_seed"),
    )?;
    call_with(
        &host_entry(py, AdapterHost::RowDiffSampling)?,
        &[
            ("row_limit", row_limit.into_bound_py_any(py)?),
            ("seed", seed.unwrap_or(0).into_bound_py_any(py)?),
        ],
    )
    .map(Some)
}

/// Invocation tolerance rules overlaid on compiled model rules.
#[pyfunction]
#[pyo3(signature = (**options))]
fn merge_row_diff_tolerances<'py>(
    options: Option<&Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    let request = Request::new("merge_row_diff_tolerances", options)?;
    let py: Python<'py> = request.py();
    let base: Bound<'py, PyAny> = request.get("base")?;
    let overlay: Bound<'py, PyAny> = request.get("override")?;
    if overlay.is_none() {
        return Ok(base);
    }
    let merged = |field: &str| -> PyResult<Bound<'py, PyAny>> {
        let combined: Bound<'py, PyDict> = PyDict::new(py);
        combined.update(native_field(&base, field)?.cast()?.as_mapping())?;
        combined.update(native_field(&overlay, field)?.cast()?.as_mapping())?;
        Ok(combined.into_any())
    };
    call_with(
        &host_entry(py, AdapterHost::RowDiffTolerances)?,
        &[
            ("by_type", merged("by_type")?),
            ("by_column", merged("by_column")?),
            ("sampling", native_field(&base, "sampling")?),
        ],
    )
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(parse_row_diff_tolerances, module)?)?;
    module.add_function(wrap_pyfunction!(parse_cli_tolerance_overrides, module)?)?;
    module.add_function(wrap_pyfunction!(resolve_row_diff_sampling, module)?)?;
    module.add_function(wrap_pyfunction!(merge_row_diff_tolerances, module)?)?;
    Ok(())
}
