//! Direct `#[getter]` reads of this module's native pyclasses, installed into `records::getters`.

use pyo3::IntoPyObjectExt;
use pyo3::prelude::{Bound, PyAny, PyResult, Python};

use crate::bindings::_helpers::planner_phases::models::{
    PlannerIdentityContext, PlannerRuntime, PlannerScopeResolution, PlannerWarehouseState,
};
use crate::bindings::_helpers::planner_scope::retained::RetainedScope;
use crate::bindings::_helpers::records::getters::class_of;
use crate::bindings::_helpers::records::models::GetterEntry;

/// The getter readers of this module's native pyclasses.
pub(crate) const GETTERS: &[GetterEntry] = &[
    GetterEntry {
        class: class_of::<RetainedScope>,
        read: read_retained_scope,
    },
    GetterEntry {
        class: class_of::<PlannerIdentityContext>,
        read: read_planner_identity_context,
    },
    GetterEntry {
        class: class_of::<PlannerRuntime>,
        read: read_planner_runtime,
    },
    GetterEntry {
        class: class_of::<PlannerScopeResolution>,
        read: read_planner_scope_resolution,
    },
    GetterEntry {
        class: class_of::<PlannerWarehouseState>,
        read: read_planner_warehouse_state,
    },
];

fn read_retained_scope<'py>(
    value: &Bound<'py, PyAny>,
    name: &str,
) -> Option<PyResult<Bound<'py, PyAny>>> {
    let Ok(cell) = value.cast::<RetainedScope>() else {
        return None;
    };
    Some(cell.get().data.field(value.py(), name))
}

fn read_planner_identity_context<'py>(
    value: &Bound<'py, PyAny>,
    name: &str,
) -> Option<PyResult<Bound<'py, PyAny>>> {
    let py: Python<'py> = value.py();
    let Ok(cell) = value.cast::<PlannerIdentityContext>() else {
        return None;
    };
    let Ok(this) = cell.try_borrow() else {
        return None;
    };
    Some(match name {
        "version_identities" => (&this.version_identities).into_bound_py_any(py),
        "stale_warning_identities" => (&this.stale_warning_identities).into_bound_py_any(py),
        _ => return None,
    })
}

fn read_planner_runtime<'py>(
    value: &Bound<'py, PyAny>,
    name: &str,
) -> Option<PyResult<Bound<'py, PyAny>>> {
    let py: Python<'py> = value.py();
    let Ok(cell) = value.cast::<PlannerRuntime>() else {
        return None;
    };
    let Ok(this) = cell.try_borrow() else {
        return None;
    };
    Some(match name {
        "project" => (&this.project).into_bound_py_any(py),
        "adapter" => (&this.adapter).into_bound_py_any(py),
        "connection" => (&this.connection).into_bound_py_any(py),
        "project_config" => (&this.project_config).into_bound_py_any(py),
        "local_config" => (&this.local_config).into_bound_py_any(py),
        "on_progress" => (&this.on_progress).into_bound_py_any(py),
        "invocation_time" => (&this.invocation_time).into_bound_py_any(py),
        _ => return None,
    })
}

fn read_planner_scope_resolution<'py>(
    value: &Bound<'py, PyAny>,
    name: &str,
) -> Option<PyResult<Bound<'py, PyAny>>> {
    let py: Python<'py> = value.py();
    let Ok(cell) = value.cast::<PlannerScopeResolution>() else {
        return None;
    };
    let Ok(this) = cell.try_borrow() else {
        return None;
    };
    Some(match name {
        "selected_scope" => (&this.selected_scope).into_bound_py_any(py),
        "stale_warning_scope" => (&this.stale_warning_scope).into_bound_py_any(py),
        "inspection_scope" => (&this.inspection_scope).into_bound_py_any(py),
        _ => return None,
    })
}

fn read_planner_warehouse_state<'py>(
    value: &Bound<'py, PyAny>,
    name: &str,
) -> Option<PyResult<Bound<'py, PyAny>>> {
    let py: Python<'py> = value.py();
    let Ok(cell) = value.cast::<PlannerWarehouseState>() else {
        return None;
    };
    let Ok(this) = cell.try_borrow() else {
        return None;
    };
    Some(match name {
        "snapshot" => (&this.snapshot).into_bound_py_any(py),
        "inspection_relations" => (&this.inspection_relations).into_bound_py_any(py),
        "migration_entries" => (&this.migration_entries).into_bound_py_any(py),
        "migration_warnings" => (&this.migration_warnings).into_bound_py_any(py),
        "column_migration_entries" => (&this.column_migration_entries).into_bound_py_any(py),
        "column_rename_hints" => (&this.column_rename_hints).into_bound_py_any(py),
        "old_name_view_entries" => (&this.old_name_view_entries).into_bound_py_any(py),
        "old_name_views" => (&this.old_name_views).into_bound_py_any(py),
        "migration_fingerprints" => (&this.migration_fingerprints).into_bound_py_any(py),
        _ => return None,
    })
}
