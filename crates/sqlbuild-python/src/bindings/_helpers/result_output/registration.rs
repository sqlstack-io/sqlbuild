//! Register the execution-result documents and the terminal event index.

use pyo3::prelude::{Bound, PyModule, PyResult};

use sqlbuild_python_output::result_output::main::events::register_event_writer::register_event_writer;
use sqlbuild_python_output::result_output::main::events::register_integration_envelope::register_integration_envelope;
use sqlbuild_python_output::result_output::main::events::register_resource_ids::register_resource_ids;
use sqlbuild_python_output::result_output::main::events::register_terminal_index::register_terminal_index;
use sqlbuild_python_output::result_output::main::results::register_aggregation::register_aggregation;
use sqlbuild_python_output::result_output::main::results::register_documents::register_documents;

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    register_terminal_index(module)?;
    register_resource_ids(module)?;
    register_aggregation(module)?;
    register_integration_envelope(module)?;
    register_event_writer(module)?;
    register_documents(module)
}
