//! A claim's identity and the ownership the launching and recovering schedulers hold for it.

use std::path::Path;

use uuid::Uuid;

/// The project directory holds Unix claim locks; the location makes Windows job names installation-specific.
#[derive(Clone, Copy, Debug)]
pub struct Claim<'a> {
    pub project_dir: &'a Path,
    pub location_id: Uuid,
    pub claim_id: Uuid,
}

/// Ownership prepared for a worker command that has not started yet.
#[derive(Debug)]
pub struct PendingOwnership {
    #[cfg(windows)]
    pub(crate) job: std::os::windows::io::OwnedHandle,
}

/// A restarted scheduler's proof of a worker that may still be running.
#[cfg(unix)]
#[derive(Debug)]
pub struct RecoveredOwnership {
    pub(crate) lock: std::fs::File,
    pub(crate) claim_id: Uuid,
}

/// A restarted scheduler's proof of a worker that may still be running.
#[cfg(windows)]
#[derive(Debug)]
pub struct RecoveredOwnership {
    pub(crate) job: Option<std::os::windows::io::OwnedHandle>,
}

/// libproc's `proc_fileinfo`, the fixed prefix of every per-descriptor record.
#[cfg(target_os = "macos")]
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct ProcFileInfo {
    pub(crate) open_flags: u32,
    pub(crate) status: u32,
    pub(crate) offset: i64,
    pub(crate) kind: i32,
    pub(crate) guard_flags: u32,
}

/// libproc's `vnode_fdinfowithpath`: the vnode a descriptor currently refers to.
#[cfg(target_os = "macos")]
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct VnodeDescriptor {
    pub(crate) file: ProcFileInfo,
    pub(crate) vnode: libc::vnode_info_path,
}
