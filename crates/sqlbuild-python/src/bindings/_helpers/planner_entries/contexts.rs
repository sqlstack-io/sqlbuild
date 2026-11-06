//! Relation, model-plan and entry-build contexts: built from keyword arguments and read by plan entry construction.

use pyo3::prelude::{Bound, Py, PyAny, PyResult, Python};
use pyo3::pymethods;
use pyo3::types::{PyDict, PyFrozenSet};

use crate::bindings::_helpers::boundary::python_calls::KeywordArguments;
use crate::bindings::_helpers::planner_entries::models::{
    ModelPlanContext, PlanEntryBuildInputs, PlannerRelationsContext,
};
use crate::bindings::_helpers::records::construct::enum_member;
use crate::bindings::_helpers::records::ids::RecordId;

/// The argument `name`, or `default()` when it was not passed.
fn value_or<'py>(
    args: &KeywordArguments<'py>,
    name: &str,
    default: impl FnOnce() -> PyResult<Bound<'py, PyAny>>,
) -> PyResult<Py<PyAny>> {
    Ok(if args.passed(name)? {
        args.value(name)?
    } else {
        default()?
    }
    .unbind())
}

fn empty_frozenset(py: Python<'_>) -> PyResult<Bound<'_, PyAny>> {
    Ok(PyFrozenSet::empty(py)?.into_any())
}

#[pymethods]
impl PlannerRelationsContext {
    #[new]
    #[pyo3(signature = (**kwargs))]
    fn new(py: Python<'_>, kwargs: Option<Bound<'_, PyDict>>) -> PyResult<Self> {
        let args = KeywordArguments::read(
            py,
            "PlannerRelationsContext",
            kwargs,
            (
                &[
                    "model_locations",
                    "seed_locations",
                    "function_locations",
                    "source_map",
                    "source_read_map",
                    "python_source_read_map",
                    "source_warehouse_columns",
                    "star_exclude_keyword",
                ],
                &["listed_source_names"],
            ),
        )?;
        Ok(Self {
            model_locations: args.value("model_locations")?.unbind(),
            seed_locations: args.value("seed_locations")?.unbind(),
            function_locations: args.value("function_locations")?.unbind(),
            source_map: args.value("source_map")?.unbind(),
            source_read_map: args.value("source_read_map")?.unbind(),
            python_source_read_map: args.value("python_source_read_map")?.unbind(),
            source_warehouse_columns: args.value("source_warehouse_columns")?.unbind(),
            star_exclude_keyword: args.value("star_exclude_keyword")?.unbind(),
            listed_source_names: value_or(&args, "listed_source_names", || empty_frozenset(py))?,
        })
    }
}

#[pymethods]
impl ModelPlanContext {
    #[new]
    #[pyo3(signature = (**kwargs))]
    fn new(py: Python<'_>, kwargs: Option<Bound<'_, PyDict>>) -> PyResult<Self> {
        let args = KeywordArguments::read(
            py,
            "ModelPlanContext",
            kwargs,
            (
                &[
                    "model_locations",
                    "models_by_name",
                    "seed_locations",
                    "function_locations",
                    "source_map",
                    "source_warehouse_columns",
                    "star_exclude_keyword",
                ],
                &[
                    "functions_by_name",
                    "runtime_cursor_producer_names",
                    "future_cursor_config",
                    "start_cursor_config",
                    "invocation_time",
                ],
            ),
        )?;
        Ok(Self {
            model_locations: args.value("model_locations")?.unbind(),
            models_by_name: args.value("models_by_name")?.into(),
            seed_locations: args.value("seed_locations")?.unbind(),
            function_locations: args.value("function_locations")?.unbind(),
            source_map: args.value("source_map")?.unbind(),
            source_warehouse_columns: args.value("source_warehouse_columns")?.unbind(),
            star_exclude_keyword: args.value("star_exclude_keyword")?.unbind(),
            functions_by_name: value_or(&args, "functions_by_name", || {
                Ok(PyDict::new(py).into_any())
            })?,
            runtime_cursor_producer_names: value_or(
                &args,
                "runtime_cursor_producer_names",
                || empty_frozenset(py),
            )?,
            future_cursor_config: args.value("future_cursor_config")?.unbind(),
            start_cursor_config: args.value("start_cursor_config")?.unbind(),
            invocation_time: args.value("invocation_time")?.unbind(),
        })
    }
}

#[pymethods]
impl PlanEntryBuildInputs {
    #[new]
    #[pyo3(signature = (**kwargs))]
    pub(crate) fn new(py: Python<'_>, kwargs: Option<Bound<'_, PyDict>>) -> PyResult<Self> {
        let args = KeywordArguments::read(
            py,
            "PlanEntryBuildInputs",
            kwargs,
            (
                &[],
                &[
                    "source_freshness_blocked_model_names",
                    "external_blocked_model_names",
                    "start_cursor_override",
                    "end_cursor_override",
                    "future_cursor_config",
                    "start_cursor_config",
                    "invocation_time",
                    "max_microbatches",
                    "max_microbatches_is_override",
                    "microbatch_limit_action",
                ],
            ),
        )?;
        Ok(Self {
            source_freshness_blocked_model_names: value_or(
                &args,
                "source_freshness_blocked_model_names",
                || empty_frozenset(py),
            )?,
            external_blocked_model_names: value_or(&args, "external_blocked_model_names", || {
                empty_frozenset(py)
            })?,
            start_cursor_override: args.value("start_cursor_override")?.unbind(),
            end_cursor_override: args.value("end_cursor_override")?.unbind(),
            future_cursor_config: args.value("future_cursor_config")?.unbind(),
            start_cursor_config: args.value("start_cursor_config")?.unbind(),
            invocation_time: args.value("invocation_time")?.unbind(),
            max_microbatches: args.value("max_microbatches")?.unbind(),
            max_microbatches_is_override: args.flag("max_microbatches_is_override")?,
            microbatch_limit_action: value_or(&args, "microbatch_limit_action", || {
                default_microbatch_limit_action(py)
            })?,
        })
    }
}

/// The microbatch limit action used when a project configures none.
pub(crate) fn default_microbatch_limit_action(py: Python<'_>) -> PyResult<Bound<'_, PyAny>> {
    enum_member(py, RecordId::MicrobatchLimitAction, "ERROR")
}
