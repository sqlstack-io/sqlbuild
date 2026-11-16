//! Direct `#[getter]` reads of this module's native pyclasses, installed into `records::getters`.

use pyo3::IntoPyObjectExt;
use pyo3::prelude::{Bound, PyAny, PyResult, Python};

use crate::bindings::_helpers::event_exporting::dispatcher::EventExporterDispatcher;
use sqlbuild_python_base::event_exporting::models::BoundEventExporter;
use sqlbuild_python_base::event_exporting::models::EventExportSummary;
use sqlbuild_python_base::event_exporting::models::EventExporterAccounting;
use sqlbuild_python_base::event_exporting::models::EventExporterCommandScope;
use sqlbuild_python_base::event_exporting::models::EventExporterCounts;
use sqlbuild_python_base::event_exporting::models::EventExporterFailure;
use sqlbuild_python_base::records::main::registry::class_of::class_of;
use sqlbuild_python_base::records::models::GetterEntry;

/// The getter readers of this module's native pyclasses.
pub(crate) const GETTERS: &[GetterEntry] = &[
    GetterEntry {
        class: class_of::<EventExporterDispatcher>,
        read: read_event_exporter_dispatcher,
    },
    GetterEntry {
        class: class_of::<BoundEventExporter>,
        read: read_bound_event_exporter,
    },
    GetterEntry {
        class: class_of::<EventExportSummary>,
        read: read_event_export_summary,
    },
    GetterEntry {
        class: class_of::<EventExporterAccounting>,
        read: read_event_exporter_accounting,
    },
    GetterEntry {
        class: class_of::<EventExporterCounts>,
        read: read_event_exporter_counts,
    },
    GetterEntry {
        class: class_of::<EventExporterFailure>,
        read: read_event_exporter_failure,
    },
    GetterEntry {
        class: class_of::<EventExporterCommandScope>,
        read: read_event_exporter_command_scope,
    },
];

fn read_event_exporter_dispatcher<'py>(
    value: &Bound<'py, PyAny>,
    name: &str,
) -> Option<PyResult<Bound<'py, PyAny>>> {
    let py: Python<'py> = value.py();
    let Ok(cell) = value.cast::<EventExporterDispatcher>() else {
        return None;
    };
    let Ok(this) = cell.try_borrow() else {
        return None;
    };
    Some(match name {
        "thread" => this.thread(py).into_bound_py_any(py),
        "notification_thread" => this.notification_thread(py).into_bound_py_any(py),
        _ => return None,
    })
}

fn read_bound_event_exporter<'py>(
    value: &Bound<'py, PyAny>,
    name: &str,
) -> Option<PyResult<Bound<'py, PyAny>>> {
    let py: Python<'py> = value.py();
    let Ok(cell) = value.cast::<BoundEventExporter>() else {
        return None;
    };
    let Ok(this) = cell.try_borrow() else {
        return None;
    };
    Some(match name {
        "name" => this.name().into_bound_py_any(py),
        "function" => this.function(py).into_bound_py_any(py),
        "provider_arguments" => this.provider_arguments(py).into_bound_py_any(py),
        "event_kinds" => this
            .event_kinds(py)
            .and_then(|value| value.into_bound_py_any(py)),
        "min_severity" => this
            .min_severity(py)
            .and_then(|value| value.into_bound_py_any(py)),
        _ => return None,
    })
}

fn read_event_export_summary<'py>(
    value: &Bound<'py, PyAny>,
    name: &str,
) -> Option<PyResult<Bound<'py, PyAny>>> {
    let py: Python<'py> = value.py();
    let Ok(cell) = value.cast::<EventExportSummary>() else {
        return None;
    };
    let Ok(this) = cell.try_borrow() else {
        return None;
    };
    Some(match name {
        "aggregate" => this.aggregate().into_bound_py_any(py),
        "per_exporter" => this
            .per_exporter(py)
            .and_then(|value| value.into_bound_py_any(py)),
        "queue_depth" => this.queue_depth().into_bound_py_any(py),
        "queue_capacity" => this.queue_capacity().into_bound_py_any(py),
        "flush_complete" => this.flush_complete().into_bound_py_any(py),
        "accepted" => this.accepted().into_bound_py_any(py),
        "filtered" => this.filtered().into_bound_py_any(py),
        "delivered" => this.delivered().into_bound_py_any(py),
        "failed" => this.failed().into_bound_py_any(py),
        "dropped" => this.dropped().into_bound_py_any(py),
        _ => return None,
    })
}

fn read_event_exporter_accounting<'py>(
    value: &Bound<'py, PyAny>,
    name: &str,
) -> Option<PyResult<Bound<'py, PyAny>>> {
    let py: Python<'py> = value.py();
    let Ok(cell) = value.cast::<EventExporterAccounting>() else {
        return None;
    };
    let Ok(this) = cell.try_borrow() else {
        return None;
    };
    Some(match name {
        "exporter_name" => this.exporter_name().into_bound_py_any(py),
        "counts" => this.counts().into_bound_py_any(py),
        _ => return None,
    })
}

fn read_event_exporter_counts<'py>(
    value: &Bound<'py, PyAny>,
    name: &str,
) -> Option<PyResult<Bound<'py, PyAny>>> {
    let py: Python<'py> = value.py();
    let Ok(cell) = value.cast::<EventExporterCounts>() else {
        return None;
    };
    let Ok(this) = cell.try_borrow() else {
        return None;
    };
    Some(match name {
        "accepted" => this.accepted().into_bound_py_any(py),
        "filtered" => this.filtered().into_bound_py_any(py),
        "delivered" => this.delivered().into_bound_py_any(py),
        "failed" => this.failed().into_bound_py_any(py),
        "dropped" => this.dropped().into_bound_py_any(py),
        _ => return None,
    })
}

fn read_event_exporter_failure<'py>(
    value: &Bound<'py, PyAny>,
    name: &str,
) -> Option<PyResult<Bound<'py, PyAny>>> {
    let py: Python<'py> = value.py();
    let Ok(cell) = value.cast::<EventExporterFailure>() else {
        return None;
    };
    let Ok(this) = cell.try_borrow() else {
        return None;
    };
    Some(match name {
        "exporter_name" => this.exporter_name().into_bound_py_any(py),
        "error_type" => this.error_type().into_bound_py_any(py),
        "event_kind" => this.event_kind().into_bound_py_any(py),
        "event_severity" => this
            .event_severity(py)
            .and_then(|value| value.into_bound_py_any(py)),
        _ => return None,
    })
}

fn read_event_exporter_command_scope<'py>(
    value: &Bound<'py, PyAny>,
    name: &str,
) -> Option<PyResult<Bound<'py, PyAny>>> {
    let py: Python<'py> = value.py();
    let Ok(cell) = value.cast::<EventExporterCommandScope>() else {
        return None;
    };
    let Ok(this) = cell.try_borrow() else {
        return None;
    };
    Some(match name {
        "dispatcher" => this.dispatcher(py).into_bound_py_any(py),
        "provider_session" => this
            .provider_session(py)
            .and_then(|value| value.into_bound_py_any(py)),
        "project_dir" => this
            .project_dir(py)
            .and_then(|value| value.into_bound_py_any(py)),
        "providers" => this
            .providers(py)
            .and_then(|value| value.into_bound_py_any(py)),
        "event_exporters" => this
            .event_exporters(py)
            .and_then(|value| value.into_bound_py_any(py)),
        "command_output_sinks" => this
            .command_output_sinks(py)
            .and_then(|value| value.into_bound_py_any(py)),
        _ => return None,
    })
}
