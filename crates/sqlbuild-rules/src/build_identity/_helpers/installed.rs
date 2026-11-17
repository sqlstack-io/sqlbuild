//! The identity the binary installed, once per process.

use std::sync::OnceLock;

/// The build identity the binary installed at startup; empty until then.
pub(crate) static INSTALLED_BUILD_IDENTITY: OnceLock<String> = OnceLock::new();
