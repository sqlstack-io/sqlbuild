//! Names and limits cross-file discovery validation checks against.

/// The prefix discovery strips from model paths before matching path defaults.
pub(crate) const MODELS_PREFIX: &str = "models/";
/// `str(Path(...).parent)` of a path with no folder.
pub(crate) const CURRENT_DIRECTORY: &str = ".";
/// How many known model folders an unmatched path default lists.
pub(crate) const KNOWN_FOLDER_LIMIT: usize = 5;
/// Model names SQLBuild reserves for compiled output structure.
pub(crate) const RESERVED_MODEL_NAMES: [&str; 1] = ["_chain_"];
/// The first seed read, grown until the header and two data rows are seen or the file ends.
pub(crate) const INITIAL_SEED_READ_BYTES: u64 = 64 * 1024;
/// A header and two data rows: enough to know a seed holds more than one data row.
pub(crate) const RECORDS_FOR_MANY_ROWS: usize = 3;

/// How a colliding orchestration declaration is renamed, added to its name conflict.
pub(crate) const ORCHESTRATION_RENAME_ADVICE: &str = " (a job or job group takes its name from \
     `name` in its TOML file, and a calendar or sensor from its `name` argument)";
