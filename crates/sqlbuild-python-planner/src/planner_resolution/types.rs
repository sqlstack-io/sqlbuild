//! Public type aliases of planner resolution.

use pyo3::prelude::{Bound, PyAny};

/// A model's resolved cursor start and end; either may be `None`.
pub type CursorBounds<'py> = (Bound<'py, PyAny>, Bound<'py, PyAny>);
