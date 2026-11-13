//! What the project fingerprint walk skips; the same folders project reuse leaves out.

/// Folders skipped only directly below the project root.
pub const EXCLUDED_ROOT_DIRECTORIES: &[&str] = &[
    "logs",
    "target",
    "venv",
    ".cache",
    ".fensu",
    ".hg",
    ".idea",
    ".mypy_cache",
    ".nox",
    ".pytest_cache",
    ".ruff_cache",
    ".sqlbuild",
    ".svn",
    ".tox",
    ".venv",
    ".vscode",
];

/// Folders skipped at any depth.
pub const EXCLUDED_DIRECTORIES: &[&str] = &[".git", "__pycache__"];

/// Files that count only by presence because every build rewrites them.
pub const PRESENCE_ONLY_FILE_SUFFIXES: &[&str] = &[".duckdb", ".duckdb.wal"];

/// Digest tag of a file whose bytes count.
pub const CONTENT_TAG: &[u8] = b"content";
/// Digest tag of a file that counts only by presence.
pub const PRESENCE_TAG: &[u8] = b"presence";

/// File suffixes whose sizes and modification times identify an editable install's code.
pub(crate) const CODE_SOURCE_SUFFIXES: &[&str] = &["py", "so", "pyd"];
/// The bytecode folder an editable install's code walk skips.
pub(crate) const BYTECODE_CACHE_DIRECTORY: &str = "__pycache__";

/// Distributions whose installed versions identify the code behind cached results.
pub const CODE_IDENTITY_DISTRIBUTIONS: [&str; 3] = ["sqlbuild", "polyglot-sql-chio", "PyYAML"];
/// The distribution an editable install's `direct_url.json` describes.
pub(crate) const SQLBUILD_DISTRIBUTION: &str = "sqlbuild";
/// The version reported for a distribution no search path holds.
pub(crate) const UNKNOWN_VERSION: &str = "unknown";
/// Metadata folder suffixes `importlib.metadata` searches, lowercased.
pub(crate) const METADATA_DIRECTORY_SUFFIXES: [&str; 2] = [".dist-info", ".egg-info"];
/// Metadata files read for a distribution's version, in order.
pub(crate) const METADATA_FILES: [&str; 2] = ["METADATA", "PKG-INFO"];
/// The installer's record of where a distribution was installed from.
pub(crate) const DIRECT_URL_FILE: &str = "direct_url.json";

/// Inputs at least this large are hashed across the rayon pool; the digest is the same either way.
pub(crate) const PARALLEL_HASH_BYTES: usize = 1 << 20;
/// The standard base64 alphabet.
pub const BASE64_ALPHABET: &[u8; 64] =
    b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
