//! Watermark inputs resolved through SQL reference lineage, and capped producers behind them.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyErr, PyResult, Python};
use pyo3::types::PyDict;

use sqlbuild_analysis::planner_strategy::errors::LineageFailure;
use sqlbuild_analysis::planner_strategy::main::capped_watermark_input::capped_watermark_input;
use sqlbuild_analysis::planner_strategy::main::lineage_reference::lineage_reference;
use sqlbuild_analysis::planner_strategy::models::{ProducerFacts, ReferenceIdentity};
use sqlbuild_analysis::planner_strategy::types::ReferenceGraph;

use crate::bindings::_helpers::compile_attachment::cursor_roles::resolve_cursor_input_roles;
use crate::bindings::_helpers::compiled_views::model_reads;
use crate::bindings::_helpers::compiled_views::resource_reads;
use crate::bindings::_helpers::model_config::typed_config::{config_text_is, model_values};
use crate::bindings::_helpers::native_host::model_lookup::ModelLookup;
use crate::bindings::_helpers::planner_strategy::microbatches::{
    CAP_ACTIONS, microbatch_limit_config,
};
use crate::bindings::_helpers::planner_strategy::values::{input_error, text};
use crate::bindings::_helpers::records::fields::field as native_field;
use crate::bindings::_helpers::records::fields::mapping_get;
use crate::bindings::_helpers::records::ids::ErrorId;

const MICROBATCH_MODE: &str = "microbatch";

/// The project's models and SQL functions by name.
struct ProjectLineage<'py> {
    models_by_name: ModelLookup,
    functions_by_name: Bound<'py, PyAny>,
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

fn references<'py>(owner: &Bound<'py, PyAny>) -> PyResult<Vec<Bound<'py, PyAny>>> {
    resource_reads::references(owner)?.try_iter()?.collect()
}

/// Whether a model's `incremental_mode` is microbatch.
fn microbatch_flag(model: &Bound<'_, PyAny>) -> PyResult<bool> {
    config_text_is(
        model.py(),
        &model_values(model)?,
        "incremental_mode",
        MICROBATCH_MODE,
    )
}

/// A model's watermark input names.
fn watermark_names(model: &Bound<'_, PyAny>) -> PyResult<Vec<String>> {
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
        let values: Bound<'py, PyAny> = native_field(&model_reads::config(&producer)?, "values")?;
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
fn lineage_error(py: Python<'_>, failure: LineageFailure<PyErr>) -> PyErr {
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

/// The reference one named input resolves to through model and SQL function references.
pub(crate) fn resolve_lineage_reference<'py>(
    model: &Bound<'py, PyAny>,
    input_name: &str,
    models_by_name: ModelLookup,
    functions_by_name: Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = model.py();
    let graph = ProjectLineage {
        models_by_name,
        functions_by_name,
    };
    let name: String = model_reads::model_name(model)?;
    lineage_reference(&graph, (&name, references(model)?), input_name)
        .map_err(|failure| lineage_error(py, failure))
}

/// Reject capped producers feeding a microbatch model's watermark inputs.
pub(crate) fn reject_capped_watermark_inputs<'py>(
    model: &Bound<'py, PyAny>,
    models_by_name: ModelLookup,
    functions_by_name: Option<Bound<'py, PyAny>>,
) -> PyResult<()> {
    let py: Python<'py> = model.py();
    if !microbatch_flag(model)? {
        return Ok(());
    }
    let functions_by_name: Bound<'py, PyAny> = match functions_by_name {
        Some(functions) if functions.is_truthy()? => functions,
        _ => PyDict::new(py).into_any(),
    };
    let graph = ProjectLineage {
        models_by_name,
        functions_by_name,
    };
    let name: String = model_reads::model_name(model)?;
    let inputs: Vec<String> = watermark_names(model)?;
    match capped_watermark_input(&graph, (&name, references(model)?), inputs) {
        Ok(None) => Ok(()),
        Ok(Some(capped)) => Err(input_error(
            py,
            ErrorId::CompileInput,
            &capped.message(),
            None,
        )),
        Err(failure) => Err(lineage_error(py, failure)),
    }
}
