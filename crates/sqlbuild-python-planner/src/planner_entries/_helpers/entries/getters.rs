//! Direct `#[getter]` reads of this module's native pyclasses, installed into `records::getters`.

use pyo3::IntoPyObjectExt;
use pyo3::prelude::{Bound, PyAny, PyResult, Python};

use crate::planner_entries::models::{
    ModelPlanContext, PlanEntryBuildInputs, PlannerRelationsContext,
};

pub(crate) fn read_model_plan_context<'py>(
    value: &Bound<'py, PyAny>,
    name: &str,
) -> Option<PyResult<Bound<'py, PyAny>>> {
    let py: Python<'py> = value.py();
    let Ok(cell) = value.cast::<ModelPlanContext>() else {
        return None;
    };
    let Ok(this) = cell.try_borrow() else {
        return None;
    };
    Some(match name {
        "model_locations" => (&this.model_locations).into_bound_py_any(py),
        "models_by_name" => (&this.models_by_name).into_bound_py_any(py),
        "seed_locations" => (&this.seed_locations).into_bound_py_any(py),
        "function_locations" => (&this.function_locations).into_bound_py_any(py),
        "source_map" => (&this.source_map).into_bound_py_any(py),
        "source_warehouse_columns" => (&this.source_warehouse_columns).into_bound_py_any(py),
        "star_exclude_keyword" => (&this.star_exclude_keyword).into_bound_py_any(py),
        "functions_by_name" => (&this.functions_by_name).into_bound_py_any(py),
        "runtime_cursor_producer_names" => {
            (&this.runtime_cursor_producer_names).into_bound_py_any(py)
        }
        "future_cursor_config" => (&this.future_cursor_config).into_bound_py_any(py),
        "start_cursor_config" => (&this.start_cursor_config).into_bound_py_any(py),
        "invocation_time" => (&this.invocation_time).into_bound_py_any(py),
        _ => return None,
    })
}

pub(crate) fn read_plan_entry_build_inputs<'py>(
    value: &Bound<'py, PyAny>,
    name: &str,
) -> Option<PyResult<Bound<'py, PyAny>>> {
    let py: Python<'py> = value.py();
    let Ok(cell) = value.cast::<PlanEntryBuildInputs>() else {
        return None;
    };
    let Ok(this) = cell.try_borrow() else {
        return None;
    };
    Some(match name {
        "source_freshness_blocked_model_names" => {
            (&this.source_freshness_blocked_model_names).into_bound_py_any(py)
        }
        "external_blocked_model_names" => {
            (&this.external_blocked_model_names).into_bound_py_any(py)
        }
        "start_cursor_override" => (&this.start_cursor_override).into_bound_py_any(py),
        "end_cursor_override" => (&this.end_cursor_override).into_bound_py_any(py),
        "future_cursor_config" => (&this.future_cursor_config).into_bound_py_any(py),
        "start_cursor_config" => (&this.start_cursor_config).into_bound_py_any(py),
        "invocation_time" => (&this.invocation_time).into_bound_py_any(py),
        "max_microbatches" => (&this.max_microbatches).into_bound_py_any(py),
        "max_microbatches_is_override" => {
            (&this.max_microbatches_is_override).into_bound_py_any(py)
        }
        "microbatch_limit_action" => (&this.microbatch_limit_action).into_bound_py_any(py),
        _ => return None,
    })
}

pub(crate) fn read_planner_relations_context<'py>(
    value: &Bound<'py, PyAny>,
    name: &str,
) -> Option<PyResult<Bound<'py, PyAny>>> {
    let py: Python<'py> = value.py();
    let Ok(cell) = value.cast::<PlannerRelationsContext>() else {
        return None;
    };
    let Ok(this) = cell.try_borrow() else {
        return None;
    };
    Some(match name {
        "model_locations" => (&this.model_locations).into_bound_py_any(py),
        "seed_locations" => (&this.seed_locations).into_bound_py_any(py),
        "function_locations" => (&this.function_locations).into_bound_py_any(py),
        "source_map" => (&this.source_map).into_bound_py_any(py),
        "source_read_map" => (&this.source_read_map).into_bound_py_any(py),
        "python_source_read_map" => (&this.python_source_read_map).into_bound_py_any(py),
        "source_warehouse_columns" => (&this.source_warehouse_columns).into_bound_py_any(py),
        "star_exclude_keyword" => (&this.star_exclude_keyword).into_bound_py_any(py),
        "listed_source_names" => (&this.listed_source_names).into_bound_py_any(py),
        _ => return None,
    })
}
