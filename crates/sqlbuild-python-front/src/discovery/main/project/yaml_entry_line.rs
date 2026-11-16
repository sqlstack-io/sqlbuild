//! The 1-based line of the outermost `- name: <name>` entry, if present.

use crate::discovery::_helpers::project::locations;

/// The 1-based line of the outermost `- name: <name>` entry, if present.
#[inline]
pub fn yaml_entry_line(contents: &str, name: &str) -> Option<usize> {
    locations::yaml_entry_line(contents, name)
}
