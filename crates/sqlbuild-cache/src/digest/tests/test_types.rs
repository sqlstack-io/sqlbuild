use std::path::{Path, PathBuf};

pub(crate) struct ContentDigestTestCase {
    pub(crate) description: &'static str,
    pub(crate) left: &'static [&'static str],
    pub(crate) right: &'static [&'static str],
    pub(crate) expected_equal: bool,
}

pub(crate) struct HexDigestTestCase {
    pub(crate) description: &'static str,
    pub(crate) parts: &'static [&'static str],
    pub(crate) expected_length: usize,
}

/// An edit applied to a fresh copy of the fingerprint test project.
pub(crate) type ProjectEdit = fn(&Path);

pub(crate) struct FingerprintProjectFilesTestCase {
    pub(crate) description: &'static str,
    pub(crate) edit: ProjectEdit,
    pub(crate) expected_changed: bool,
}

pub(crate) struct FingerprintFailureTestCase {
    pub(crate) description: &'static str,
    pub(crate) root: fn(&Path) -> PathBuf,
    pub(crate) expected_failure: bool,
}

pub(crate) struct DigestFilesTestCase {
    pub(crate) description: &'static str,
    pub(crate) path: fn(&Path) -> PathBuf,
    pub(crate) expected_digest_of: Option<&'static str>,
}

/// A package tree, the facts hashed before it, and Python's `installed_code_identity` digest.
pub(crate) struct InstalledCodeIdentityTestCase {
    pub(crate) description: &'static str,
    /// `(relative path, contents)`, given modification times in sorted path order.
    pub(crate) files: &'static [(&'static str, &'static str)],
    pub(crate) facts: &'static [&'static str],
    pub(crate) editable: bool,
    pub(crate) expected_identity: &'static str,
}

pub(crate) struct CompiledCodeIdentityTestCase {
    pub(crate) description: &'static str,
    pub(crate) installed: &'static str,
    pub(crate) build_identity: &'static str,
    pub(crate) expected_identity: &'static str,
}

/// Search path folders with their files, and the facts `importlib.metadata` reads from them.
pub(crate) struct InstalledCodeFactsTestCase {
    pub(crate) description: &'static str,
    /// `(search path index, relative path, contents)`.
    pub(crate) files: &'static [(usize, &'static str, &'static str)],
    pub(crate) expected_versions: [&'static str; 2],
    pub(crate) expected_editable: bool,
}
