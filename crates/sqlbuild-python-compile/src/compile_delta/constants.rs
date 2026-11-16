//! Public constants of compile delta.

/// Set to `1` to always compile the whole project.
pub const DELTA_DISABLE_ENV_VAR: &str = "SQLBUILD_DISABLE_DELTA_COMPILE";

/// Set to `1` to run each delta compile beside a full compile and fail on any difference.
pub const DELTA_VERIFY_ENV_VAR: &str = "SQLBUILD_VERIFY_DELTA_COMPILE";

/// Changes whenever the image's meaning changes, so an older image is never read.
pub const IMAGE_FORMAT: &str = "sqlbuild-project-image-v9";

pub const GATE_SECTION: &str = "gate";

pub const FACTS_SECTION: &str = "facts";

pub const RULE_DIGESTS_SECTION: &str = "rule-digests";

/// The exit code of a compile whose delta differed from its full compile.
pub const MISMATCH_EXIT_CODE: i64 = 70;
