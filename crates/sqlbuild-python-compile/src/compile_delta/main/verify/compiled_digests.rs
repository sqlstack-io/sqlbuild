//! Every file under `root` with its native digest, keyed by absolute path text.

use crate::compile_delta::_helpers::verify::files_under;
use sqlbuild_cache::digest::main::digest_files::digest_files;
use sqlbuild_cache::digest::main::hex_digest::hex_digest;
use sqlbuild_cache::project_reuse::constants::NATIVE_DIGEST_PREFIX;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Every file under `root` with its native digest, keyed by absolute path text.
pub fn compiled_digests(root: &Path) -> BTreeMap<String, String> {
    let paths: Vec<PathBuf> = files_under(root);
    let digests = digest_files(&paths);
    paths
        .into_iter()
        .zip(digests)
        .filter_map(|(path, digest)| {
            let Ok(digest) = digest else {
                return None;
            };
            Some((
                path.to_string_lossy().into_owned(),
                format!("{NATIVE_DIGEST_PREFIX}{}", hex_digest(&digest)),
            ))
        })
        .collect()
}
