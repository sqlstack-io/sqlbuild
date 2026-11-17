//! Register the native Recall classes.

use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};

use sqlbuild_python_front::discovery::main::python_files::install_recall_writer::install_recall_writer;
use sqlbuild_python_output::recall::models::RecallLifecycleSink;
use sqlbuild_python_output::recall::models::RecallWriter;

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<RecallLifecycleSink>()?;
    install_recall_writer(&module.py().get_type::<RecallWriter>());
    module.add_class::<RecallWriter>()
}
