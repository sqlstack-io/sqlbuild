//! A compiled audit's attributes the planner reads, from its native view.

use pyo3::IntoPyObjectExt;
use pyo3::prelude::{Bound, PyAny, PyResult, Python};

use crate::bindings::_helpers::compiled_views::audit_views::NativeCompiledAudit;
use sqlbuild_python_base::compile_driver::constants::COMPILED_VIEW_FALLBACKS;
use sqlbuild_python_base::compile_driver::main::count_stage::count_stage;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// The native view behind an audit object; any other object is a Python-built record, counted.
#[inline]
fn native_audit<'a>(audit: &'a Bound<'_, PyAny>) -> PyResult<Option<&'a NativeCompiledAudit>> {
    if let Ok(view) = audit.cast::<NativeCompiledAudit>() {
        return Ok(Some(view.get()));
    }
    count_stage(audit.py(), COMPILED_VIEW_FALLBACKS, 1)?;
    Ok(None)
}

/// The compiled audit's `always_run`, as its Python attribute reads it.
pub(crate) fn always_run<'py>(audit: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = audit.py();
    if let Some(view) = native_audit(audit)? {
        return view
            .always_run(py)
            .and_then(|value| value.into_bound_py_any(py));
    }
    native_field(audit, "always_run")
}

/// The compiled audit's `attached_column_name`, as its Python attribute reads it.
pub(crate) fn attached_column_name<'py>(audit: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = audit.py();
    if let Some(view) = native_audit(audit)? {
        return view
            .attached_column_name(py)
            .and_then(|value| value.into_bound_py_any(py));
    }
    native_field(audit, "attached_column_name")
}

/// The compiled audit's `attached_target_kind`, as its Python attribute reads it.
pub(crate) fn attached_target_kind<'py>(audit: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = audit.py();
    if let Some(view) = native_audit(audit)? {
        return view
            .attached_target_kind(py)
            .and_then(|value| value.into_bound_py_any(py));
    }
    native_field(audit, "attached_target_kind")
}

/// The compiled audit's `attached_target_name`, as its Python attribute reads it.
pub(crate) fn attached_target_name<'py>(audit: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = audit.py();
    if let Some(view) = native_audit(audit)? {
        return view
            .attached_target_name(py)
            .and_then(|value| value.into_bound_py_any(py));
    }
    native_field(audit, "attached_target_name")
}

/// The compiled audit's `definition_name`, as its Python attribute reads it.
pub(crate) fn definition_name<'py>(audit: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = audit.py();
    if let Some(view) = native_audit(audit)? {
        return view
            .definition_name(py)
            .and_then(|value| value.into_bound_py_any(py));
    }
    native_field(audit, "definition_name")
}

/// The compiled audit's `description`, as its Python attribute reads it.
pub(crate) fn description<'py>(audit: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = audit.py();
    if let Some(view) = native_audit(audit)? {
        return view
            .description(py)
            .and_then(|value| value.into_bound_py_any(py));
    }
    native_field(audit, "description")
}

/// The compiled audit's `evaluation_mode`, as its Python attribute reads it.
pub(crate) fn evaluation_mode<'py>(audit: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = audit.py();
    if let Some(view) = native_audit(audit)? {
        return view
            .evaluation_mode(py)
            .and_then(|value| value.into_bound_py_any(py));
    }
    native_field(audit, "evaluation_mode")
}

/// The compiled audit's `evidence_limit`, as its Python attribute reads it.
pub(crate) fn evidence_limit<'py>(audit: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = audit.py();
    if let Some(view) = native_audit(audit)? {
        return view
            .evidence_limit(py)
            .and_then(|value| value.into_bound_py_any(py));
    }
    native_field(audit, "evidence_limit")
}

/// The compiled audit's `evidence_sql`, as its Python attribute reads it.
pub(crate) fn evidence_sql<'py>(audit: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = audit.py();
    if let Some(view) = native_audit(audit)? {
        return view
            .evidence_sql(py)
            .and_then(|value| value.into_bound_py_any(py));
    }
    native_field(audit, "evidence_sql")
}

/// The compiled audit's `key`, as its Python attribute reads it.
pub(crate) fn key<'py>(audit: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = audit.py();
    if let Some(view) = native_audit(audit)? {
        return view.key(py).and_then(|value| value.into_bound_py_any(py));
    }
    native_field(audit, "key")
}

/// The compiled audit's `measurement_contract`, as its Python attribute reads it.
pub(crate) fn measurement_contract<'py>(audit: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = audit.py();
    if let Some(view) = native_audit(audit)? {
        return view
            .measurement_contract(py)
            .and_then(|value| value.into_bound_py_any(py));
    }
    native_field(audit, "measurement_contract")
}

/// The compiled audit's `meta`, as its Python attribute reads it.
pub(crate) fn meta<'py>(audit: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = audit.py();
    if let Some(view) = native_audit(audit)? {
        return view.meta(py).and_then(|value| value.into_bound_py_any(py));
    }
    native_field(audit, "meta")
}

/// The compiled audit's `minimum_samples`, as its Python attribute reads it.
pub(crate) fn minimum_samples<'py>(audit: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = audit.py();
    if let Some(view) = native_audit(audit)? {
        return view
            .minimum_samples(py)
            .and_then(|value| value.into_bound_py_any(py));
    }
    native_field(audit, "minimum_samples")
}

/// The compiled audit's `name`, as its Python attribute reads it.
pub(crate) fn name<'py>(audit: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = audit.py();
    if let Some(view) = native_audit(audit)? {
        return view.name(py).and_then(|value| value.into_bound_py_any(py));
    }
    native_field(audit, "name")
}

/// The compiled audit's `references`, as its Python attribute reads it.
pub(crate) fn references<'py>(audit: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = audit.py();
    if let Some(view) = native_audit(audit)? {
        return view
            .references(py)
            .and_then(|value| value.into_bound_py_any(py));
    }
    native_field(audit, "references")
}

/// The compiled audit's `run_scope`, as its Python attribute reads it.
pub(crate) fn run_scope<'py>(audit: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = audit.py();
    if let Some(view) = native_audit(audit)? {
        return view
            .run_scope(py)
            .and_then(|value| value.into_bound_py_any(py));
    }
    native_field(audit, "run_scope")
}

/// The compiled audit's `scope_deps`, as its Python attribute reads it.
pub(crate) fn scope_deps<'py>(audit: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = audit.py();
    if let Some(view) = native_audit(audit)? {
        return view
            .scope_deps(py)
            .and_then(|value| value.into_bound_py_any(py));
    }
    native_field(audit, "scope_deps")
}

/// The compiled audit's `severity`, as its Python attribute reads it.
pub(crate) fn severity<'py>(audit: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = audit.py();
    if let Some(view) = native_audit(audit)? {
        return view
            .severity(py)
            .and_then(|value| value.into_bound_py_any(py));
    }
    native_field(audit, "severity")
}

/// The compiled audit's `sql_body`, as its Python attribute reads it.
pub(crate) fn sql_body<'py>(audit: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = audit.py();
    if let Some(view) = native_audit(audit)? {
        return view
            .sql_body(py)
            .and_then(|value| value.into_bound_py_any(py));
    }
    native_field(audit, "sql_body")
}

/// The compiled audit's `thresholds`, as its Python attribute reads it.
pub(crate) fn thresholds<'py>(audit: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = audit.py();
    if let Some(view) = native_audit(audit)? {
        return view
            .thresholds(py)
            .and_then(|value| value.into_bound_py_any(py));
    }
    native_field(audit, "thresholds")
}
