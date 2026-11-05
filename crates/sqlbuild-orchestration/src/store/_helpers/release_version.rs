//! The release recorded on control events; only the release crates know it.

use std::sync::OnceLock;

pub(crate) static RELEASE_VERSION: OnceLock<&'static str> = OnceLock::new();
