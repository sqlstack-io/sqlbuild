//! The semantic diagnostic help catalogue, for Python's tests and corpus tooling.

use pyo3::pyfunction;
use sqlbuild_analysis::semantic_checks::main::semantic_help_catalogue::semantic_help_catalogue as catalogue;

/// Each semantic diagnostic code and its help.
#[pyfunction]
pub(crate) fn semantic_help_catalogue() -> Vec<(&'static str, &'static str)> {
    catalogue().to_vec()
}
