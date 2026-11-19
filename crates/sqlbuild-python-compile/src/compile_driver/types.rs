//! Public traits and type aliases of compile driver.

use crate::compile_driver::models::CommandOutcome;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;

/// One compile's outcome, input reads and artifact writes, before it is stored and printed.
pub type PhasesOutcome<'py> = (CommandOutcome<'py>, Bound<'py, PyAny>, Bound<'py, PyAny>);

/// Pipeline result fields by name.
pub type PipelineFields<'py> = Vec<(&'static str, pyo3::Bound<'py, pyo3::PyAny>)>;
/// The jobs whose models a build rebuilds, and each job with the warehouse its nodes run on.
pub type JobRunContext = (Vec<String>, Vec<(String, String)>);
