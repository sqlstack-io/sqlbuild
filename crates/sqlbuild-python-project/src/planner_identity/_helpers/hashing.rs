//! Query and function definition fingerprints for change detection, failing as planner input errors.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyErr, PyResult, Python};
use pyo3::types::PyString;
use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::PoisonError;

use sqlbuild_analysis::planner_identity::constants::{FINGERPRINT_HELP, QUERY_FINGERPRINT_FAILED};
use sqlbuild_analysis::sql_tokens::main::baseline_fingerprint::baseline_fingerprint;
use sqlbuild_analysis::sql_tokens::main::compiler_query_fingerprint::compiler_query_fingerprint;
use sqlbuild_core::panics::main::is_compiler_panic::is_compiler_panic;

use sqlbuild_python_base::boundary::main::compiler_error::compiler_error;
use sqlbuild_python_base::boundary::main::keywords::keywords;
use sqlbuild_python_base::records::main::construct::error_class::error_class;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::ErrorId;
use sqlbuild_python_base::types::CompilerDetach;

/// Query fingerprints by dialect and SQL text, reused across a plan and a location's later previews.
pub(crate) static QUERY_FINGERPRINTS: Mutex<Option<QueryFingerprints>> = Mutex::new(None);
/// Distinct SQL texts the memo keeps before starting over; a 10K-model project holds about 10K.
const QUERY_FINGERPRINTS_LIMIT: usize = 100_000;

/// Fingerprint results by dialect, then by SQL text, so a lookup borrows the SQL text.
pub(crate) type QueryFingerprints = Vec<(Option<String>, HashMap<String, Result<String, String>>)>;

/// The remembered fingerprint of `sql` under `dialect`, if any.
fn remembered(
    memo: &Mutex<Option<QueryFingerprints>>,
    sql: &str,
    dialect: Option<&str>,
) -> Option<Result<String, String>> {
    memo.lock()
        .unwrap_or_else(PoisonError::into_inner)
        .as_ref()?
        .iter()
        .find(|(known, _)| known.as_deref() == dialect)?
        .1
        .get(sql)
        .cloned()
}

/// `compiler_query_fingerprint`, remembered by dialect and SQL text.
pub(crate) fn query_fingerprint(
    memo: &Mutex<Option<QueryFingerprints>>,
    sql: &str,
    dialect: Option<&str>,
) -> Result<String, String> {
    if let Some(known) = remembered(memo, sql, dialect) {
        return known;
    }
    let fingerprint: Result<String, String> = compiler_query_fingerprint(sql, dialect);
    let mut memo = memo.lock().unwrap_or_else(PoisonError::into_inner);
    let dialects: &mut QueryFingerprints = memo.get_or_insert_with(Vec::new);
    if dialects.iter().map(|(_, texts)| texts.len()).sum::<usize>() > QUERY_FINGERPRINTS_LIMIT {
        dialects.clear();
    }
    match dialects
        .iter_mut()
        .find(|(known, _)| known.as_deref() == dialect)
    {
        Some((_, texts)) => {
            texts.insert(sql.to_owned(), fingerprint.clone());
        }
        None => dialects.push((
            dialect.map(str::to_owned),
            HashMap::from([(sql.to_owned(), fingerprint.clone())]),
        )),
    }
    fingerprint
}

/// The query fingerprint of a model, or the planner input error naming the model.
pub(crate) fn model_query_hash(
    py: Python<'_>,
    model_name: &str,
    (query_sql, dialect): (&str, Option<&str>),
) -> PyResult<String> {
    match remembered(&QUERY_FINGERPRINTS, query_sql, dialect) {
        Some(known) => known,
        None => py.compiler_detach(|| query_fingerprint(&QUERY_FINGERPRINTS, query_sql, dialect)),
    }
    .map_err(|error| fingerprint_error(py, &format!("Model '{model_name}' SQL"), error))
}

/// `current`, or the fingerprint recorded with a `Fingerprint` row whose definition fingerprints to `current`.
pub(crate) fn recorded_baseline(
    py: Python<'_>,
    current: String,
    recorded: &Bound<'_, PyAny>,
    fingerprint: impl FnOnce(&str) -> Result<String, String> + Send,
) -> PyResult<String> {
    if recorded.is_none() {
        return Ok(current);
    }
    let definition: String = native_field(recorded, "definition")?.extract()?;
    let recorded_hash: String = native_field(recorded, "definition_hash")?.extract()?;
    if recorded_hash == current || recorded_hash.is_empty() || definition.is_empty() {
        return Ok(current);
    }
    py.compiler_detach(|| {
        Ok(baseline_fingerprint(
            current,
            (&definition, &recorded_hash),
            fingerprint,
        ))
    })
    .map_err(compiler_error)
}

pub(crate) fn fingerprint_error(py: Python<'_>, subject: &str, message: String) -> PyErr {
    if is_compiler_panic(&message) {
        return compiler_error(message);
    }
    let text = |value: &str| PyString::new(py, value).into_any();
    let raised: PyResult<Bound<'_, PyAny>> =
        error_class(py, ErrorId::PlannerInput).and_then(|class| {
            class.call(
                (format!(
                    "{subject} cannot be fingerprinted for change detection: {message}"
                ),),
                Some(&keywords(
                    py,
                    &[
                        ("code", text(QUERY_FINGERPRINT_FAILED)),
                        ("help", text(FINGERPRINT_HELP)),
                    ],
                )?),
            )
        });
    match raised {
        Ok(error) => PyErr::from_value(error),
        Err(error) => error,
    }
}
