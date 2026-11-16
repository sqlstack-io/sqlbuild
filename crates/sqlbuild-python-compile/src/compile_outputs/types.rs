//! Public traits and type aliases of compile outputs.

/// A written artifact's digest, or a kept artifact's size and mtime.
pub type ArtifactRow = (Option<String>, Option<u64>, Option<i64>);
