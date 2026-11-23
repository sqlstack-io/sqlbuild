//! The scope-relevant config files, roots and declaration folders of every project.

use sqlbuild_discovery::constants::{CANONICAL_AUTHORED_ROOTS, GLOBAL_DECLARATION_DIRECTORIES};

use crate::scope_cache::constants::{
    EXTRA_SOURCE_ROOTS, FINGERPRINT_CONFIG_FILES, NAMED_DECLARATION_DIRECTORIES,
};
use crate::scope_cache::models::FingerprintInputs;
use crate::scope_index::constants::GLOBAL_ROLE_PARENT;

/// The fingerprint inputs of the project layout, for the running SQLBuild version.
#[must_use]
pub fn project_fingerprint_inputs(sqlbuild_version: String) -> FingerprintInputs {
    let mut declaration_directories: Vec<String> = owned(&GLOBAL_DECLARATION_DIRECTORIES);
    declaration_directories.sort();
    let named_declaration_directories: Vec<String> = owned(&NAMED_DECLARATION_DIRECTORIES);
    let roots: Vec<String> = CANONICAL_AUTHORED_ROOTS
        .iter()
        .map(|parts| parts.join("/"))
        .chain(
            declaration_directories
                .iter()
                .chain(&named_declaration_directories)
                .map(|name| format!("{GLOBAL_ROLE_PARENT}/{name}")),
        )
        .chain(owned(&EXTRA_SOURCE_ROOTS))
        .collect();
    FingerprintInputs {
        config_files: owned(&FINGERPRINT_CONFIG_FILES),
        roots,
        declaration_directories,
        named_declaration_directories,
        sqlbuild_version,
    }
}

fn owned(names: &[&str]) -> Vec<String> {
    names.iter().map(|name| (*name).to_owned()).collect()
}
