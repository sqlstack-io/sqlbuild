//! Path-default glob segments.

/// A segment matching exactly one path segment.
pub const SINGLE_SEGMENT_GLOB: &str = "*";
/// A segment matching any number of path segments.
pub const RECURSIVE_SEGMENT_GLOB: &str = "**";
/// The project-relative folder prefix model paths drop before matching.
pub const MODELS_PREFIX: &str = "sql/models/";
/// The help a path-default conflict shows.
pub const CONFLICT_HELP: &str = "Make one pattern narrower by adding literal or '*' path segments, \
or remove the overlap. Path-default selection never depends on declaration order.";
