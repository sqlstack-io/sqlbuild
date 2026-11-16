//! Which compile of this process may reuse stored macro calls: only its first.

use std::sync::{Mutex, MutexGuard, PoisonError};

use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::{pyfunction, wrap_pyfunction};

use sqlbuild_python_base::compile_context::main::delta::injection::injection;

/// Whether a compile claimed the first-compile slot, and whether a delta compile only borrowed it.
#[derive(Debug)]
struct FirstCompile {
    claimed: bool,
    lent_to_delta: bool,
}

static FIRST_COMPILE: Mutex<FirstCompile> = Mutex::new(FirstCompile {
    claimed: false,
    lent_to_delta: false,
});

fn locked<T>(slot: &Mutex<T>) -> MutexGuard<'_, T> {
    slot.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Record that a compile started; true only for this process's first compile invocation.
#[pyfunction]
pub(crate) fn claim_first_compile() -> bool {
    let mut first: MutexGuard<'_, FirstCompile> = locked(&FIRST_COMPILE);
    if injection().is_some() {
        let granted: bool = !first.claimed || first.lent_to_delta;
        first.claimed = true;
        first.lent_to_delta = granted;
        return granted;
    }
    if first.lent_to_delta {
        first.lent_to_delta = false;
        return true;
    }
    let granted: bool = !first.claimed;
    first.claimed = true;
    granted
}

/// Keep the delta compile's claim: it finished, so no full compile of this invocation follows.
pub(crate) fn keep_delta_claim() {
    locked(&FIRST_COMPILE).lent_to_delta = false;
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(claim_first_compile, module)?)
}
