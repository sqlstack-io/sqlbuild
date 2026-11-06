//! Fixed authorization texts that callers may see.

/// A refused listing names no node, so a caller cannot learn nodes it cannot view or whether a location exists.
pub(crate) const LISTING_DENIED: &str = "missing view on one or more nodes in this location";
