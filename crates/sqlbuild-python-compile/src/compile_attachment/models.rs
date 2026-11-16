//! Public records of compile attachment.

use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;

/// Where SQL expansion inputs come from: the project, its discovery and the scope to reuse.
#[derive(Debug)]
pub struct ExpansionSources<'py> {
    pub project_dir: Bound<'py, PyAny>,
    pub value_renderer: Bound<'py, PyAny>,
    pub vars: ExpansionVars<'py>,
    pub discovered_inputs: Option<Bound<'py, PyAny>>,
    pub declaration_scope: Option<Bound<'py, PyAny>>,
    pub static_declaration_scope: Option<Bound<'py, PyAny>>,
}

/// Where expansion takes its target and vars from.
#[derive(Debug)]
pub enum ExpansionVars<'py> {
    /// For commands without `--target`: the project's default target, with these CLI vars.
    ProjectDefault(Option<Bound<'py, PyAny>>),
    /// The compile's effective target name and vars, exactly as the command resolved them.
    Compile {
        target_name: Bound<'py, PyAny>,
        vars: Bound<'py, PyAny>,
    },
}
