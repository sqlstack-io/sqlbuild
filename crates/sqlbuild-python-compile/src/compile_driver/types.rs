//! Public traits and type aliases of compile driver.

use crate::compile_driver::models::CommandOutcome;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;

/// One compile's outcome, input reads and artifact writes, before it is stored and printed.
pub type PhasesOutcome<'py> = (CommandOutcome<'py>, Bound<'py, PyAny>, Bound<'py, PyAny>);
