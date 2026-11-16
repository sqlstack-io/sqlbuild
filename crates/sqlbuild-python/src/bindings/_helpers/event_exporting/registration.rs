//! Register native lifecycle-event export and stop its threads at interpreter exit.

use pyo3::prelude::{Bound, PyAnyMethods, PyModule, PyModuleMethods, PyResult, wrap_pyfunction};

use crate::bindings::_helpers::event_exporting::dispatcher::{
    EventExporterDispatcher, stop_event_exporter_threads,
};
use crate::bindings::_helpers::event_exporting::policy;
use crate::bindings::_helpers::event_exporting::workers::DispatcherWorker;
use sqlbuild_python_base::boundary::main::imported::imported;
use sqlbuild_python_base::event_exporting::main::register_scope::register_scope;
use sqlbuild_python_base::event_exporting::models::BoundEventExporter;
use sqlbuild_python_base::event_exporting::models::EventExportSummary;
use sqlbuild_python_base::event_exporting::models::EventExporterAccounting;
use sqlbuild_python_base::event_exporting::models::EventExporterCounts;
use sqlbuild_python_base::event_exporting::models::EventExporterFailure;

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<EventExporterCounts>()?;
    module.add_class::<EventExporterAccounting>()?;
    module.add_class::<EventExportSummary>()?;
    module.add_class::<EventExporterFailure>()?;
    module.add_class::<BoundEventExporter>()?;
    module.add_class::<EventExporterDispatcher>()?;
    module.add_class::<DispatcherWorker>()?;
    let hook = wrap_pyfunction!(stop_event_exporter_threads, module)?;
    module.add_function(hook.clone())?;
    imported(module.py(), "atexit", "register")?.call1((hook,))?;
    policy::register(module)?;
    register_scope(module)
}
