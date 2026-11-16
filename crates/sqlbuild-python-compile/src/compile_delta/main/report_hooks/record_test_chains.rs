//! Record which models each SQL test's compiled artifact is keyed by.

use crate::compile_delta::_helpers::report_hooks::record_test_chains_inner;
use pyo3::prelude::{Bound, PyAny, PyResult};
use sqlbuild_python_base::compile_context::main::delta::guarded::guarded;

/// Record which models each SQL test's compiled artifact is keyed by.
pub fn record_test_chains(tests: &Bound<'_, PyAny>, chains: &Bound<'_, PyAny>) -> PyResult<()> {
    guarded(|| record_test_chains_inner(tests, chains))
}
