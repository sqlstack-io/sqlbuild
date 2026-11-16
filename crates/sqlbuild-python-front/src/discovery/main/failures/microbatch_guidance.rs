//! Microbatch guidance.

use pyo3::pyfunction;
use sqlbuild_config::project::main::microbatch_guidance::microbatch_guidance as concurrency_guidance;

#[pyfunction]
pub fn microbatch_guidance() -> (String, String) {
    concurrency_guidance()
}
