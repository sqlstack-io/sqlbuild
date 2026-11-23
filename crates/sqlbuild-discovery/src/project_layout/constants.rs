//! The entries a project may hold without listing them, and the folders the check reads.

/// Folders SQLBuild itself writes into a project: build artifacts and logs.
pub(crate) const OUTPUT_DIRECTORIES: [&str; 2] = ["target", "logs"];
/// Suffixes of local warehouse files a DuckDB or SQLite connection keeps beside the project.
pub(crate) const DATABASE_FILE_SUFFIXES: [&str; 4] = [".duckdb", ".db", ".sqlite", ".sqlite3"];
/// Suffixes of the write-ahead log, shared-memory and journal files beside a database file.
pub(crate) const DATABASE_COMPANION_SUFFIXES: [&str; 4] = [".wal", "-wal", "-shm", "-journal"];
/// Python's bytecode cache folder.
pub(crate) const PYTHON_CACHE_DIRECTORY: &str = "__pycache__";
/// The prefix of a hidden entry, such as `.gitignore`, `.env` or `.agents/`.
pub(crate) const HIDDEN_PREFIX: &str = ".";
/// Git's ignore file name.
pub(crate) const GITIGNORE_FILE: &str = ".gitignore";
/// The folder marking a Git repository root.
pub(crate) const GIT_DIRECTORY: &str = ".git";
/// A repository's own exclude file, below its `.git` folder.
pub(crate) const GIT_EXCLUDE_FILE: [&str; 2] = ["info", "exclude"];
