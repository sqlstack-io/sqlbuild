//! Type aliases shared by result output.

use pyo3::prelude::{Bound, PyAny};
use sqlbuild_host::models::ScenarioRunResult;

/// Scenario results as the executor returns them.
pub type ScenarioResults<'py> = Vec<ScenarioRunResult<Bound<'py, PyAny>>>;
