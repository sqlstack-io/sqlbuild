//! Explicit references help.

use pyo3::pyfunction;
use sqlbuild_config::project::main::explicit_references_help::explicit_references_help as references_help;

#[pyfunction]
#[pyo3(signature = (*, allowed))]
pub fn explicit_references_help(allowed: &str) -> String {
    references_help(allowed)
}
