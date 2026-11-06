//! Build `BuildExecutionResult` with aggregate counts and overall status from typed results.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyModule, PyModuleMethods, PyResult, Python};
use pyo3::types::PyDict;
use pyo3::{IntoPyObjectExt, pyfunction, wrap_pyfunction};
use sqlbuild_core::build_aggregate::main::aggregate_build_counts::aggregate_build_counts;
use sqlbuild_core::build_aggregate::models::{BuildCounts, BuildResultFacts, ModelResultFacts};
use sqlbuild_host::entries::models::ExecutorHost;
use sqlbuild_host::models::{
    AuditExecutionResult, BuildExecutionResult, FunctionExecutionResult, LoadExecutionResult,
    ModelExecutionResult, SeedExecutionResult, SqlTestExecutionResult,
};
use sqlbuild_host::types::ResultEnum;

use crate::bindings::_helpers::boundary::keyword_request::Request;
use crate::bindings::_helpers::boundary::python_calls::call_with;
use crate::bindings::_helpers::native_host::host;
use crate::bindings::_helpers::records::fields::field as native_field;
use crate::bindings::_helpers::result_output::projection::Projection;

/// Borrowed executor results with canonical terminals, used by the build JSON projection.
pub(crate) struct RetainedBuildResults<'a, V> {
    pub(crate) models: Vec<&'a ModelExecutionResult<V>>,
    pub(crate) seeds: Vec<&'a SeedExecutionResult>,
    pub(crate) functions: Vec<&'a FunctionExecutionResult>,
    pub(crate) loads: Vec<&'a LoadExecutionResult<V>>,
    pub(crate) tests: Vec<&'a SqlTestExecutionResult<V>>,
    pub(crate) source_audits: Vec<&'a AuditExecutionResult<V>>,
    pub(crate) end_audits: Vec<&'a AuditExecutionResult<V>>,
}

impl<'a, V> RetainedBuildResults<'a, V> {
    pub(crate) fn new(
        result: &'a BuildExecutionResult<V>,
        projection: &Projection<'_>,
    ) -> PyResult<Self> {
        Ok(Self {
            models: projection.retained(&result.model_results)?,
            seeds: projection.retained(&result.seed_results)?,
            functions: projection.retained(&result.function_results)?,
            loads: projection.retained(&result.load_results)?,
            tests: projection.retained(&result.test_results)?,
            source_audits: projection.retained(&result.source_audit_results)?,
            end_audits: projection.retained(&result.end_audit_results)?,
        })
    }

    pub(crate) fn is_partial(&self, result: &BuildExecutionResult<V>) -> bool {
        [
            self.models.len(),
            self.seeds.len(),
            self.functions.len(),
            self.loads.len(),
            self.tests.len(),
            self.source_audits.len(),
            self.end_audits.len(),
        ] != [
            result.model_results.len(),
            result.seed_results.len(),
            result.function_results.len(),
            result.load_results.len(),
            result.test_results.len(),
            result.source_audit_results.len(),
            result.end_audit_results.len(),
        ]
    }

    pub(crate) fn counts(&self) -> BuildCounts {
        let mut models: Vec<ModelResultFacts> = Vec::new();
        for model in &self.models {
            let audit_outcomes: Vec<String> = model
                .audit_results
                .iter()
                .map(|audit| audit.outcome.value().to_owned())
                .collect();
            models.push(ModelResultFacts {
                status: model.status.value().to_owned(),
                error_code: model.error_code.clone(),
                warning_count: model.warning_messages.len(),
                audit_outcomes,
            });
        }
        aggregate_build_counts(&BuildResultFacts {
            models,
            seed_statuses: self
                .seeds
                .iter()
                .map(|result| result.status.value().to_owned())
                .collect(),
            functions: self
                .functions
                .iter()
                .map(|result| {
                    (
                        result.status.value().to_owned(),
                        result.warning_messages.len(),
                    )
                })
                .collect(),
            load_statuses: self
                .loads
                .iter()
                .map(|result| result.status.value().to_owned())
                .collect(),
            test_outcomes: self
                .tests
                .iter()
                .map(|result| result.outcome.value().to_owned())
                .collect(),
            source_audit_outcomes: self
                .source_audits
                .iter()
                .map(|result| result.outcome.value().to_owned())
                .collect(),
            end_audit_outcomes: self
                .end_audits
                .iter()
                .map(|result| result.outcome.value().to_owned())
                .collect(),
        })
    }
}

const RESULT_FIELDS: [&str; 7] = [
    "model_results",
    "seed_results",
    "function_results",
    "load_results",
    "test_results",
    "source_audit_results",
    "end_audit_results",
];

fn texts(results: &Bound<'_, PyAny>, field: &str) -> PyResult<Vec<String>> {
    results
        .try_iter()?
        .map(|result| result?.getattr(field)?.str()?.extract())
        .collect()
}

fn facts(sections: &[Bound<'_, PyAny>]) -> PyResult<BuildResultFacts> {
    let models: Vec<ModelResultFacts> = sections[0]
        .try_iter()?
        .map(|model| {
            let model: Bound<'_, PyAny> = model?;
            Ok(ModelResultFacts {
                status: native_field(&model, "status")?.str()?.extract()?,
                error_code: native_field(&model, "error_code")?.extract()?,
                warning_count: native_field(&model, "warning_messages")?.len()?,
                audit_outcomes: texts(&native_field(&model, "audit_results")?, "outcome")?,
            })
        })
        .collect::<PyResult<_>>()?;
    let functions: Vec<(String, usize)> = sections[2]
        .try_iter()?
        .map(|function| {
            let function: Bound<'_, PyAny> = function?;
            Ok((
                native_field(&function, "status")?.str()?.extract()?,
                native_field(&function, "warning_messages")?.len()?,
            ))
        })
        .collect::<PyResult<_>>()?;
    Ok(BuildResultFacts {
        models,
        seed_statuses: texts(&sections[1], "status")?,
        functions,
        load_statuses: texts(&sections[3], "status")?,
        test_outcomes: texts(&sections[4], "outcome")?,
        source_audit_outcomes: texts(&sections[5], "outcome")?,
        end_audit_outcomes: texts(&sections[6], "outcome")?,
    })
}

/// `BuildExecutionResult` over the seven result tuples, in `RESULT_FIELDS` order.
pub(crate) fn aggregated_build_result<'py>(
    py: Python<'py>,
    sections: &[Bound<'py, PyAny>],
) -> PyResult<Bound<'py, PyAny>> {
    let counts: BuildCounts = aggregate_build_counts(&facts(sections)?);
    let status: Bound<'py, PyAny> = host::entry(py, ExecutorHost::BuildStatus)?
        .getattr(if counts.failed { "FAILED" } else { "SUCCESS" })?;
    let mut keywords: Vec<(&str, Bound<'py, PyAny>)> = vec![("status", status)];
    keywords.extend(RESULT_FIELDS.iter().copied().zip(sections.iter().cloned()));
    keywords.extend([
        ("success_count", counts.success.into_bound_py_any(py)?),
        ("failure_count", counts.failure.into_bound_py_any(py)?),
        ("timeout_count", counts.timeout.into_bound_py_any(py)?),
        ("skipped_count", counts.skipped.into_bound_py_any(py)?),
        ("warning_count", counts.warning.into_bound_py_any(py)?),
        (
            "insufficient_count",
            counts.insufficient.into_bound_py_any(py)?,
        ),
    ]);
    call_with(
        &host::entry(py, ExecutorHost::BuildExecutionResult)?,
        &keywords,
    )
}

/// Compute aggregate counts and overall build status.
#[pyfunction]
#[pyo3(signature = (**options))]
fn aggregate_build_result<'py>(
    options: Option<&Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    let request = Request::new("aggregate_build_result", options)?;
    let sections: Vec<Bound<'py, PyAny>> = RESULT_FIELDS
        .iter()
        .map(|field| request.get(field))
        .collect::<PyResult<_>>()?;
    aggregated_build_result(request.py(), &sections)
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(aggregate_build_result, module)?)?;
    Ok(())
}
