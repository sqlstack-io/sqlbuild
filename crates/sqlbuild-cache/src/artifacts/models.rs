//! What one artifact write or publication did.

use std::path::PathBuf;

/// The files a write batch replaced and left unchanged.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct WrittenArtifacts {
    pub written: usize,
    pub unchanged: usize,
}

/// How staged artifacts reached the compiled directory.
#[derive(Debug, PartialEq, Eq)]
pub enum Publication {
    /// The staged files differ from the expected set; nothing was published.
    StagedChanged,
    /// The whole staged directory became the compiled directory.
    MovedTree,
    /// Each staged file was published at its compiled path, in this order.
    Files(Vec<(PathBuf, PathBuf)>),
}

/// What reusing one compiled SQL test artifact requires: its identity, place and content.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SqlTestArtifactRecord {
    /// The digest of everything the artifact was rendered from.
    pub identity: String,
    /// The artifact's path below the compiled tests directory.
    pub relative_path: PathBuf,
    pub size: u64,
    pub mtime_ns: i64,
    /// The lowercase hex SHA-256 of the artifact's bytes.
    pub content_sha256: String,
}

/// Why a cached SQL test artifact could not be kept.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SqlTestRecordMiss {
    IdentityChanged,
    UnsafePath,
    ArtifactMissing,
    SizeChanged,
    UnreadableMtime,
    ContentChanged,
}

impl SqlTestRecordMiss {
    /// The reason as a short snake-case label for debug logs.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::IdentityChanged => "identity_changed",
            Self::UnsafePath => "unsafe_path",
            Self::ArtifactMissing => "artifact_missing",
            Self::SizeChanged => "size_changed",
            Self::UnreadableMtime => "unreadable_mtime",
            Self::ContentChanged => "content_changed",
        }
    }
}

impl SqlTestArtifactRecord {
    /// The relative path's `as_posix()` text, as the cache file stores it.
    #[must_use]
    pub fn relative_posix(&self) -> String {
        self.relative_path
            .components()
            .map(|component| component.as_os_str().to_string_lossy().into_owned())
            .collect::<Vec<String>>()
            .join("/")
    }
}
