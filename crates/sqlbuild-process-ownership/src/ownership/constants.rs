//! Names and numbers shared by the launching and the recovering scheduler.

/// Every owned worker carries its claim ID here, so a stop never reaches another claim's process.
pub const WORKER_CLAIM_ENV: &str = "SQLBUILD_WORKER_CLAIM";

/// Keep inherited claim ownership away from standard IO and the runtime's usual low descriptors.
#[cfg(unix)]
pub(crate) const CLAIM_FD: i32 = 198;

/// libproc's vnode-with-path flavour for one descriptor, absent from the libc bindings.
#[cfg(target_os = "macos")]
pub(crate) const PROC_PIDFDVNODEPATHINFO: i32 = 2;

/// A session-local name keeps other sessions out; the account-only DACL keeps other users out.
#[cfg(windows)]
pub(crate) const JOB_PREFIX: &str = "Local\\sqlbuild-worker-";
