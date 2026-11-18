//! Values a location's runtime keeps between requests.

use pyo3::prelude::{Bound, PyAny};

/// The discovered inputs and `CompiledProjectPhaseResult` of the project a location last published.
pub(crate) type HeldProject<'py> = (Bound<'py, PyAny>, Bound<'py, PyAny>);
