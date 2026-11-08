//! The analysis cache context and the model analysis cache metrics.

use pyo3::exceptions::{PyOSError, PyTypeError, PyValueError};
use pyo3::prelude::{
    Bound, PyAny, PyAnyMethods, PyErr, PyModule, PyModuleMethods, PyResult, Python,
};
use pyo3::types::{PyBool, PyDict, PyDictMethods, PyList, PyListMethods, PySlice, PyString};
use pyo3::{pyfunction, wrap_pyfunction};
use sqlbuild_cache::digest::main::sha256_hex::sha256_hex;
use sqlbuild_host::compile_context::main::record_compile_metric::record_compile_metric;

use crate::bindings::_helpers::boundary::python_calls::{KeywordArguments, imported};
use crate::bindings::_helpers::planner_identity::json_values::{COMPACT, unicode_sorted_dumps};
use crate::bindings::_helpers::records::construct::new_record_with;
use crate::bindings::_helpers::records::fields::field as native_field;
use crate::bindings::_helpers::records::ids::RecordId;

const ANALYSIS_CACHE_VERSION: i64 = 13;
const ANALYSIS_ALGORITHM_FINGERPRINT: &str = "model-sql-analysis-v21-completed-star-shapes";
const LOCAL_QUALNAME_MARKER: &str = "<locals>";
const PACKAGE_NAME: &str = "sqlbuild";
const UNKNOWN_VERSION: &str = "unknown";
/// Sorted; the dialects whose expression types compile checks.
const TYPE_CHECKED_DIALECTS: [&str; 4] = ["bigquery", "duckdb", "postgresql", "snowflake"];
const METADATA_MODULE: &str = "importlib.metadata";
const CONTEXT_REQUIRED: [&str; 3] = ["root", "inference_profile", "allow_compact_analysis"];
const CONTEXT_OPTIONAL: [&str; 2] = ["rich_type_inference", "signature_namespace"];

/// A reusable cache context, or `None` when the profile identity is unstable.
pub(crate) fn analysis_cache_context<'py>(
    root: &Bound<'py, PyAny>,
    inference_profile: &Bound<'py, PyAny>,
    (allow_compact_analysis, rich_type_inference): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
    signature_namespace: &Bound<'py, PyAny>,
) -> PyResult<Option<Bound<'py, PyAny>>> {
    let py: Python<'py> = root.py();
    if root.is_none() {
        return Ok(None);
    }
    let Some(profile) = inference_profile_payload(inference_profile)? else {
        return Ok(None);
    };
    let shared: Bound<'py, PyDict> = PyDict::new(py);
    shared.set_item("algorithm", ANALYSIS_ALGORITHM_FINGERPRINT)?;
    shared.set_item("cache_version", ANALYSIS_CACHE_VERSION)?;
    shared.set_item("sqlbuild_version", package_version(py, "sqlbuild")?)?;
    shared.set_item(
        "python_version",
        imported(py, "platform", "python_version_tuple")?
            .call0()?
            .get_item(PySlice::new(py, 0, 2, 1))?,
    )?;
    shared.set_item("allow_compact_analysis", allow_compact_analysis)?;
    shared.set_item("rich_type_inference", rich_type_inference)?;
    shared.set_item(
        "type_checked_dialects",
        PyList::new(py, TYPE_CHECKED_DIALECTS)?,
    )?;
    shared.set_item("inference_profile", profile)?;
    let digests = (|| -> PyResult<(String, String)> {
        Ok((
            payload_digest(shared.as_any())?,
            payload_digest(signature_namespace)?,
        ))
    })();
    let (shared_fingerprint, namespace) = match digests {
        Ok(digests) => digests,
        Err(error) if json_failure(py, &error) => return Ok(None),
        Err(error) => return Err(error),
    };
    let context = new_record_with(
        py,
        RecordId::AnalysisCacheContext,
        &[
            ("root", root.clone()),
            (
                "shared_fingerprint",
                PyString::new(py, &shared_fingerprint).into_any(),
            ),
            (
                "signature_namespace",
                PyString::new(py, &namespace).into_any(),
            ),
        ],
    );
    match context {
        Ok(context) => Ok(Some(context)),
        Err(error) if json_failure(py, &error) => Ok(None),
        Err(error) => Err(error),
    }
}

/// Build a reusable cache context, or bypass when profile identity is unstable.
#[pyfunction]
#[pyo3(signature = (**options))]
fn build_analysis_cache_context<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<Option<Bound<'py, PyAny>>> {
    let arguments: KeywordArguments<'py> = KeywordArguments::read(
        py,
        "build_analysis_cache_context",
        options,
        (&CONTEXT_REQUIRED, &CONTEXT_OPTIONAL),
    )?;
    let rich: Bound<'py, PyAny> = if arguments.passed("rich_type_inference")? {
        arguments.value("rich_type_inference")?
    } else {
        PyBool::new(py, true).to_owned().into_any()
    };
    analysis_cache_context(
        &arguments.value("root")?,
        &arguments.value("inference_profile")?,
        (&arguments.value("allow_compact_analysis")?, &rich),
        &arguments.value("signature_namespace")?,
    )
}

/// Semantic analysis cache outcome counts for one invocation.
pub(crate) struct AnalysisCacheCounts {
    pub(crate) batch_hits: usize,
    pub(crate) entry_hits: usize,
    pub(crate) misses: usize,
    pub(crate) bypasses: usize,
}

/// Record semantic analysis cache outcomes for one invocation.
pub(crate) fn record_analysis_cache_metrics(
    AnalysisCacheCounts {
        batch_hits,
        entry_hits,
        misses,
        bypasses,
    }: AnalysisCacheCounts,
) {
    for (metric, value) in [
        ("analysis_batch_cache_hits", batch_hits),
        ("analysis_entry_cache_hits", entry_hits),
        ("analysis_cache_misses", misses),
        ("analysis_cache_bypasses", bypasses),
    ] {
        record_compile_metric(metric, i64::try_from(value).unwrap_or(i64::MAX));
    }
}

/// Whether `error` is the `TypeError` or `ValueError` an unstable payload raises.
fn json_failure(py: Python<'_>, error: &PyErr) -> bool {
    error.is_instance_of::<PyTypeError>(py) || error.is_instance_of::<PyValueError>(py)
}

/// The profile identity payload, or `None` when a nullability rule is not a stable module function.
fn inference_profile_payload<'py>(
    profile: &Bound<'py, PyAny>,
) -> PyResult<Option<Bound<'py, PyDict>>> {
    let py: Python<'py> = profile.py();
    let inspect = py.import("inspect")?;
    let sorted: Bound<'py, PyAny> = imported(py, "builtins", "sorted")?;
    let rules: Bound<'py, PyList> = PyList::empty(py);
    let items: Bound<'py, PyAny> =
        sorted.call1((
            native_field(profile, "function_nullability_rules")?.call_method0("items")?,
        ))?;
    for item in items.try_iter()? {
        let (name, rule): (Bound<'py, PyAny>, Bound<'py, PyAny>) = item?.extract()?;
        if !inspect.call_method1("isfunction", (&rule,))?.is_truthy()? {
            return Ok(None);
        }
        let module: Bound<'py, PyAny> = rule.getattr("__module__")?;
        let qualname: Bound<'py, PyAny> = rule.getattr("__qualname__")?;
        let stable: bool = module
            .extract::<String>()?
            .strip_prefix(PACKAGE_NAME)
            .is_some_and(|rest| rest.starts_with('.'))
            && !qualname
                .extract::<String>()?
                .contains(LOCAL_QUALNAME_MARKER)
            && rule.getattr("__closure__")?.is_none()
            && rule.getattr("__defaults__")?.is_none()
            && rule.getattr("__kwdefaults__")?.is_none();
        if !stable {
            return Ok(None);
        }
        let source: String = match inspect.call_method1("getsource", (&rule,)) {
            Ok(source) => source.extract()?,
            Err(error)
                if error.is_instance_of::<PyOSError>(py)
                    || error.is_instance_of::<PyTypeError>(py) =>
            {
                return Ok(None);
            }
            Err(error) => return Err(error),
        };
        let entry: Bound<'py, PyDict> = PyDict::new(py);
        entry.set_item("name", name)?;
        entry.set_item("module", module)?;
        entry.set_item("qualname", qualname)?;
        entry.set_item("source_digest", sha256_hex(source.as_bytes()))?;
        rules.append(entry)?;
    }
    let payload: Bound<'py, PyDict> = PyDict::new(py);
    for field in [
        "sql_analysis_dialect",
        "quoted_identifiers_ignore_case",
        "semantic_known_functions",
        "semantic_known_types",
    ] {
        payload.set_item(field, native_field(profile, field)?)?;
    }
    payload.set_item("function_nullability_rules", rules)?;
    payload.set_item(
        "function_return_types",
        imported(py, "builtins", "dict")?.call1((sorted.call1((native_field(
            profile,
            "function_return_types",
        )?
        .call_method0("items")?,))?,))?,
    )?;
    Ok(Some(payload))
}

/// The SHA-256 of `json.dumps(payload, ensure_ascii=False, separators=(",", ":"), sort_keys=True)`.
fn payload_digest(payload: &Bound<'_, PyAny>) -> PyResult<String> {
    let encoded: String = unicode_sorted_dumps(payload, COMPACT)?;
    Ok(sha256_hex(encoded.as_bytes()))
}

/// `importlib.metadata.version(package)`, or `unknown` when the package is not installed.
fn package_version(py: Python<'_>, package: &str) -> PyResult<String> {
    match imported(py, METADATA_MODULE, "version")?.call1((package,)) {
        Ok(version) => version.extract(),
        Err(error)
            if error.is_instance(py, &imported(py, METADATA_MODULE, "PackageNotFoundError")?) =>
        {
            Ok(UNKNOWN_VERSION.to_owned())
        }
        Err(error) => Err(error),
    }
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(build_analysis_cache_context, module)?)
}
