//! The relation lineage cache key.

use std::path::PathBuf;

use pyo3::prelude::{PyResult, Python};
use pyo3::pyfunction;
use sqlbuild_analysis::lineage::main::relation_fingerprint::relation_fingerprint;
use sqlbuild_analysis::lineage::models::InterruptedListingPolicy;

use sqlbuild_python_base::boundary::main::compiler_error::compiler_error;
use sqlbuild_python_base::boundary::main::native_compiler_error::native_compiler_error;
use sqlbuild_python_base::types::CompilerDetach;

/// The authored files' digest, or `None`; the flag is true where `rglob` raises mid-listing.
#[pyfunction]
pub(crate) fn relation_lineage_fingerprint(
    py: Python<'_>,
    project_dir: PathBuf,
    prefix: Vec<u8>,
    interrupted_listing_uncacheable: bool,
) -> PyResult<Option<String>> {
    let policy = if interrupted_listing_uncacheable {
        InterruptedListingPolicy::Uncacheable
    } else {
        InterruptedListingPolicy::Skip
    };
    let fingerprint = py
        .compiler_detach(|| Ok(relation_fingerprint(&project_dir, &prefix, policy)))
        .map_err(compiler_error)?;
    fingerprint.into_digest().map_err(|message| {
        native_compiler_error(format!(
            "NativeCompilerError: relation lineage fingerprint failed: {message}"
        ))
    })
}
