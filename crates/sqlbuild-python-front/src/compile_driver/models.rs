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

/// The project's connections by use.
#[derive(Debug, Default)]
pub struct ConnectionRoles {
    /// The source connections, with SQLLoad's keys.
    pub sources: Vec<SourceConnection>,
    /// The SQLBuild connections, each with whether it sets `kind`.
    pub warehouses: Vec<(String, bool)>,
}

/// One source connection: its merged settings and the file that defines it.
#[derive(Debug)]
pub struct SourceConnection {
    pub name: String,
    pub file: &'static str,
    pub settings: toml::Table,
}
