//! The 1-based line of the outermost `- name: <name>` entry, if present.

use pyo3::pyfunction;
use sqlbuild_discovery::yaml_files::main::yaml_entry_lines::yaml_entry_lines;

/// The 1-based line of the outermost `- name: <name>` entry, if present.
#[pyfunction]
#[pyo3(signature = (*, contents, name))]
pub fn yaml_entry_line(contents: &str, name: &str) -> Option<usize> {
    yaml_entry_lines(contents)
        .into_iter()
        .find_map(|(entry, line)| (entry == name).then_some(line))
}
