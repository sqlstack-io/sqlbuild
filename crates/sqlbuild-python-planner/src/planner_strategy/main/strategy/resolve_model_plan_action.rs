//! The plan action and reason for one model.

use crate::planner_changes::main::planner_enum::planner_enum;
use crate::planner_strategy::_helpers::strategy::action_value;
use crate::planner_strategy::_helpers::strategy::reason_value;
use crate::planner_strategy::_helpers::values::{
    change_facts, config_value, model_materialization, optional_string,
};
use crate::planner_strategy::main::values::input_error::input_error;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::{PyBool, PyTuple};
use sqlbuild_analysis::planner_strategy::main::model_plan_action::model_plan_action;
use sqlbuild_analysis::planner_strategy::models::ModelStrategy;
use sqlbuild_python_base::records::types::ErrorId;
use sqlbuild_python_base::records::types::RecordId;

/// The plan action and reason for one model.
#[pyfunction]
#[pyo3(signature = (*, model, change_result, full_refresh))]
pub fn resolve_model_plan_action<'py>(
    model: &Bound<'py, PyAny>,
    change_result: &Bound<'py, PyAny>,
    full_refresh: bool,
) -> PyResult<Bound<'py, PyTuple>> {
    let py: Python<'py> = model.py();
    let enabled: Bound<'py, PyAny> = config_value(model, "enabled")?;
    let strategy = ModelStrategy {
        name:
            sqlbuild_python_project::compiled_views::main::compiled_models::model_name::model_name(
                model,
            )?,
        disabled: enabled.is_instance_of::<PyBool>() && !enabled.is_truthy()?,
        materialization: model_materialization(model)?,
        incremental_strategy: optional_string(&config_value(model, "incremental_strategy")?)?,
    };
    let (action, reason) =
        model_plan_action(&strategy, &change_facts(change_result)?, full_refresh).map_err(
            |error| {
                input_error(
                    py,
                    ErrorId::PlannerInput,
                    &error.message(),
                    Some(error.code()),
                )
            },
        )?;
    PyTuple::new(
        py,
        [
            planner_enum(py, RecordId::PlanAction, action_value(action))?,
            planner_enum(py, RecordId::PlanReason, reason_value(reason))?,
        ],
    )
}
