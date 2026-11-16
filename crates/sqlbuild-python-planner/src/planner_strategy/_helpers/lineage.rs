//! Watermark inputs resolved through SQL reference lineage, and capped producers behind them.

use crate::planner_strategy::main::values::input_error::input_error;
use crate::planner_strategy::main::values::text::text;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyErr, PyResult, Python};

use sqlbuild_analysis::planner_strategy::errors::LineageFailure;
use sqlbuild_analysis::planner_strategy::models::{ProducerFacts, ReferenceIdentity};
use sqlbuild_analysis::planner_strategy::types::ReferenceGraph;

use crate::planner_strategy::_helpers::microbatches::{CAP_ACTIONS, microbatch_limit_config};
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::main::fields::mapping_get::mapping_get;
use sqlbuild_python_base::records::types::ErrorId;
use sqlbuild_python_front::model_config::main::config::config_text_is::config_text_is;
use sqlbuild_python_front::model_config::main::config::model_values::model_values;
use sqlbuild_python_project::compile_attachment::main::resolve_cursor_input_roles::resolve_cursor_input_roles;
use sqlbuild_python_project::compiled_views::main::compiled_models::config::config;
use sqlbuild_python_project::native_host::models::ModelLookup;

const MICROBATCH_MODE: &str = "microbatch";

/// The project's models and SQL functions by name.
pub(crate) struct ProjectLineage<'py> {
    pub(crate) models_by_name: ModelLookup,
    pub(crate) functions_by_name: Bound<'py, PyAny>,
}

/// The references of the value under `name`, when the mapping has it.
fn mapped_references<'py>(
    mapping: &Bound<'py, PyAny>,
    name: &str,
) -> PyResult<Option<Vec<Bound<'py, PyAny>>>> {
    let value: Bound<'py, PyAny> = mapping_get(mapping, name)?;
    if value.is_none() {
        return Ok(None);
    }
    references(&value).map(Some)
}

pub(crate) fn references<'py>(owner: &Bound<'py, PyAny>) -> PyResult<Vec<Bound<'py, PyAny>>> {
    sqlbuild_python_project::resource_views::main::resource_reads::references::references(owner)?
        .try_iter()?
        .collect()
}

/// Whether a model's `incremental_mode` is microbatch.
pub(crate) fn microbatch_flag(model: &Bound<'_, PyAny>) -> PyResult<bool> {
    config_text_is(
        model.py(),
        &model_values(model)?,
        "incremental_mode",
        MICROBATCH_MODE,
    )
}

/// A model's watermark input names.
pub(crate) fn watermark_names(model: &Bound<'_, PyAny>) -> PyResult<Vec<String>> {
    native_field(&resolve_cursor_input_roles(model)?, "watermark_inputs")?
        .try_iter()?
        .map(|name| text(&name?))
        .collect()
}

impl<'py> ReferenceGraph for ProjectLineage<'py> {
    type Reference = Bound<'py, PyAny>;
    type Error = PyErr;

    fn identity(&self, reference: &Bound<'py, PyAny>) -> PyResult<ReferenceIdentity> {
        Ok(ReferenceIdentity {
            kind: text(&native_field(reference, "ref_kind")?)?,
            name: text(&native_field(reference, "ref_name")?)?,
        })
    }

    fn model_references(&self, name: &str) -> PyResult<Option<Vec<Bound<'py, PyAny>>>> {
        let value = self.models_by_name.get(self.functions_by_name.py(), name)?;
        if value.is_none() {
            return Ok(None);
        }
        references(&value).map(Some)
    }

    fn function_references(&self, name: &str) -> PyResult<Option<Vec<Bound<'py, PyAny>>>> {
        mapped_references(&self.functions_by_name, name)
    }

    fn producer_facts(&self, name: &str) -> PyResult<ProducerFacts> {
        let producer = self.models_by_name.get(self.functions_by_name.py(), name)?;
        let values: Bound<'py, PyAny> = native_field(&config(&producer)?, "values")?;
        let capped: bool = match microbatch_limit_config(&values)?.1 {
            Some(action) => CAP_ACTIONS.contains(&text(&action)?.as_str()),
            None => false,
        };
        Ok(ProducerFacts {
            microbatch: microbatch_flag(&producer)?,
            capped,
        })
    }

    fn watermark_inputs(&self, name: &str) -> PyResult<Vec<String>> {
        watermark_names(&self.models_by_name.get(self.functions_by_name.py(), name)?)
    }
}

/// The Python error of a failed lineage walk.
pub(crate) fn lineage_error(py: Python<'_>, failure: LineageFailure<PyErr>) -> PyErr {
    match failure {
        LineageFailure::Graph(error) => error,
        LineageFailure::NotInLineage { model, input } => input_error(
            py,
            ErrorId::PlannerInput,
            &LineageFailure::<PyErr>::message(&model, &input),
            Some(LineageFailure::<PyErr>::code()),
        ),
    }
}
