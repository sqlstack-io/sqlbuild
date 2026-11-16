//! Compile only the changed files against the stored compile, or say why it cannot.

use crate::compile_command::constants::TARGET_DIRECTORY_NAME;
use crate::compile_delta::_helpers::driver::AFTER_FAILED_RULES;
use crate::compile_delta::_helpers::driver::Base;
use crate::compile_delta::_helpers::driver::Prepared;
use crate::compile_delta::_helpers::driver::Rerun;
use crate::compile_delta::_helpers::driver::Round;
use crate::compile_delta::_helpers::driver::fallback;
use crate::compile_delta::_helpers::driver::finished;
use crate::compile_delta::_helpers::driver::ineligible;
use crate::compile_delta::_helpers::driver::prepared;
use crate::compile_delta::_helpers::driver::rounds;
use crate::compile_delta::main::driver::delta_enabled::delta_enabled;
use crate::compile_delta::main::driver::trace::trace;
use crate::compile_delta::models::DeltaOutcome;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use sqlbuild_cache::project_reuse::constants::FAILED_COMPILED_DIRECTORY_NAME;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use std::path::PathBuf;

/// Compile only the changed files against the stored compile, or say why it cannot.
pub fn run<'py>(
    request: &Bound<'py, PyAny>,
    attempt: &Bound<'py, PyAny>,
) -> PyResult<DeltaOutcome<'py>> {
    if !delta_enabled() {
        return fallback("delta compiles are off");
    }
    if let Some(reason) = ineligible(request)? {
        return fallback(reason);
    }
    let project_dir: PathBuf = native_field(attempt, "project_dir")?.extract()?;
    if project_dir
        .join(TARGET_DIRECTORY_NAME)
        .join(FAILED_COMPILED_DIRECTORY_NAME)
        .is_dir()
    {
        return fallback(AFTER_FAILED_RULES);
    }
    let py: Python<'py> = request.py();
    let started: std::time::Instant = std::time::Instant::now();
    let prepared: Prepared = match prepared(request, attempt)? {
        Ok(prepared) => prepared,
        Err(reason) => return fallback(reason),
    };
    trace(py, "delta base read", started)?;
    let changed_files: usize = prepared.changed_files;
    let (ran, rerun, base): (Round, Rerun, Base) = match rounds(request, attempt, prepared)? {
        Ok(found) => found,
        Err(outcome) => return Ok(outcome),
    };
    trace(py, "delta rounds", started)?;
    let outcome: PyResult<DeltaOutcome<'py>> = finished(ran, base, &rerun, (py, changed_files));
    trace(py, "delta report", started)?;
    outcome
}
