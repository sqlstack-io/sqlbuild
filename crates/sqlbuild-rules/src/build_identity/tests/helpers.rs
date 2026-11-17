use crate::build_identity::_helpers::installed::INSTALLED_BUILD_IDENTITY;
use crate::build_identity::main::native_build_identity::native_build_identity;

/// Set only on the fresh process the missing-identity test starts, so other runs of ignored tests skip the read.
pub(crate) const FRESH_PROCESS_ENV_VAR: &str = "SQLBUILD_RULES_FRESH_IDENTITY_PROCESS";

/// In the fresh process, whether an identity was installed before a read that panics without one; elsewhere `false`.
pub(crate) fn read_identity_in_fresh_process() -> bool {
    std::env::var_os(FRESH_PROCESS_ENV_VAR)
        .map(|_| {
            let installed: bool = INSTALLED_BUILD_IDENTITY.get().is_some();
            let _ = native_build_identity();
            installed
        })
        .unwrap_or(false)
}
