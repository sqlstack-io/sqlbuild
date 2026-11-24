//! Directory names and limits of the paths SQLBuild writes under `target/`.

pub(crate) const CHAIN_DIR: &str = "_chain_";
pub(crate) const SQL_FILE_SUFFIX: &str = ".sql";
/// A model chain directory name longer than this many bytes is shortened to a digest.
pub(crate) const MAX_CHAIN_COMPONENT_BYTES: usize = 200;
pub(crate) const CHAIN_DIGEST_LENGTH: usize = 12;
/// Bytes kept from the first and last model names of a shortened chain directory.
pub(crate) const CHAIN_EDGE_NAME_BYTES: usize = 80;
pub(crate) const MODELS_DIR: &str = sqlbuild_core::constants::MODELS_PATH;
pub(crate) const FUNCTIONS_DIR: &str = sqlbuild_core::constants::FUNCTIONS_PATH;
