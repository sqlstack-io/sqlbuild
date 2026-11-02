//! Semantic type equality for native callers, with the adapter wrapper's parse-fallback debug event.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock, PoisonError};

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::PyString;
use sqlbuild_analysis::type_system::main::normalize_type::normalize_type;
use sqlbuild_analysis::type_system::main::same_type::same_type;
use sqlbuild_analysis::type_system::models::{NormalizedType, TypeNormalization};
use sqlbuild_host::entries::models::ExecutorHost;

use crate::bindings::_helpers::boundary::panics::compiler_guard;
use crate::bindings::_helpers::boundary::python_calls::call_with;
use crate::bindings::_helpers::native_host::host;
use crate::bindings::_helpers::type_system::normalization::normalization_error;

const TYPE_NORMALIZATION_LOGGER: &str = "sqlbuild.adapter";
const TYPE_PARSE_FALLBACK_MESSAGE: &str = "type normalization polyglot parse failed; falling back";
const DEFAULT_DIALECT: &str = "generic";
const CACHE_LIMIT: usize = 1024;

type Cache = Mutex<HashMap<(String, String), NormalizedType>>;

static NORMALIZED: OnceLock<Cache> = OnceLock::new();

/// Whether two type strings are semantically equal under `dialect` (a `TypeDialect`, name or `None`).
pub(crate) fn types_equal(dialect: &Bound<'_, PyAny>, left: &str, right: &str) -> PyResult<bool> {
    let name: String = if dialect.is_truthy()? {
        dialect.str()?.extract()?
    } else {
        DEFAULT_DIALECT.to_owned()
    };
    let left_type: NormalizedType = normalized(dialect, &name, left)?;
    let right_type: NormalizedType = normalized(dialect, &name, right)?;
    Ok(same_type(&left_type, &right_type, &name))
}

fn normalized(dialect: &Bound<'_, PyAny>, name: &str, type_sql: &str) -> PyResult<NormalizedType> {
    let py: Python<'_> = dialect.py();
    let cache: &Cache = NORMALIZED.get_or_init(|| Mutex::new(HashMap::new()));
    let key: (String, String) = (type_sql.to_owned(), name.to_owned());
    let cached: Option<NormalizedType> = cache
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .get(&key)
        .cloned();
    if let Some(found) = cached {
        return Ok(found);
    }
    let TypeNormalization {
        normalized,
        parse_error,
    } = compiler_guard(|| Ok(normalize_type(type_sql, name)))?
        .map_err(|error| normalization_error(py, &error))?;
    if let Some(parse_error) = parse_error {
        log_parse_fallback(dialect, type_sql, &parse_error)?;
    }
    let mut held = cache.lock().unwrap_or_else(PoisonError::into_inner);
    if held.len() >= CACHE_LIMIT {
        held.clear();
    }
    held.insert(key, normalized.clone());
    Ok(normalized)
}

fn log_parse_fallback(
    dialect: &Bound<'_, PyAny>,
    type_sql: &str,
    parse_error: &str,
) -> PyResult<()> {
    let py: Python<'_> = dialect.py();
    call_with(
        &host::entry(py, ExecutorHost::LogDebugEvent)?,
        &[
            (
                "logger",
                py.import("logging")?
                    .getattr("getLogger")?
                    .call1((TYPE_NORMALIZATION_LOGGER,))?,
            ),
            (
                "message",
                PyString::new(py, TYPE_PARSE_FALLBACK_MESSAGE).into_any(),
            ),
            ("type_sql", PyString::new(py, type_sql).into_any()),
            ("dialect", dialect.str()?.into_any()),
            ("sqlbuild_error", PyString::new(py, parse_error).into_any()),
        ],
    )?;
    Ok(())
}
