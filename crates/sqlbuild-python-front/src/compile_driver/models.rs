//! Public records of compile driver.

use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;

/// The loaded configuration of one project.
#[derive(Debug)]
pub struct ProjectConfiguration<'a, 'py> {
    pub project_dir: &'a Bound<'py, PyAny>,
    pub project_config: &'a Bound<'py, PyAny>,
    pub local_config: &'a Bound<'py, PyAny>,
}
