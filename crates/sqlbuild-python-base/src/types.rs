use pyo3::prelude::{Bound, PyAny, PyResult, Python};
use pyo3::types::PyTuple;

pub trait CompilerDetach {
    fn compiler_detach<T: Send, F: FnOnce() -> Result<T, String> + Send>(
        self,
        operation: F,
    ) -> Result<T, String>;
}

/// A named record read from the positional tuple Python sends.
pub trait FromPositional: Sized {
    fn from_positional(value: &Bound<'_, PyAny>) -> PyResult<Self>;
}

/// A named record converted into the positional tuple Python receives.
pub trait IntoPositional {
    fn into_positional<'py>(self, py: Python<'py>) -> PyResult<Bound<'py, PyTuple>>;
}
