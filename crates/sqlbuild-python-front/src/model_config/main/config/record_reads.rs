//! Record native template environment and context reads for project reuse, in order.

use crate::model_config::_helpers::config::template_data::ENVIRONMENT_READ;
use pyo3::prelude::{PyResult, Python};
use sqlbuild_python_base::compile_context::main::input_reads::context_read::context_read;
use sqlbuild_python_base::compile_context::main::input_reads::environment_read::environment_read;

/// Record native template environment and context reads for project reuse, in order.
pub fn record_reads<'a>(
    py: Python<'_>,
    reads: impl IntoIterator<Item = (&'a str, &'a str)>,
) -> PyResult<()> {
    for (kind, name) in reads {
        if kind == ENVIRONMENT_READ {
            environment_read(py, name)?;
        } else {
            context_read(py, name)?;
        }
    }
    Ok(())
}
