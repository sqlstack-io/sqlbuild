//! Which compile of this process may reuse stored macro calls: only its first.

use std::sync::{Mutex, MutexGuard, PoisonError};

use pyo3::pyfunction;

use sqlbuild_python_base::compile_context::main::delta::injection::injection;

/// Whether a compile claimed the first-compile slot, and whether a delta compile only borrowed it.
#[derive(Debug)]
pub(crate) struct FirstCompile {
    claimed: bool,
    pub(crate) lent_to_delta: bool,
}

pub(crate) static FIRST_COMPILE: Mutex<FirstCompile> = Mutex::new(FirstCompile {
    claimed: false,
    lent_to_delta: false,
});

pub(crate) fn locked<T>(slot: &Mutex<T>) -> MutexGuard<'_, T> {
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
