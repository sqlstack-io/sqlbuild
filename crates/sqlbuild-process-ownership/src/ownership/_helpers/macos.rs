//! macOS has no pidfd: only a PID reused in the microseconds after the last recheck could be hit.

use std::mem::{MaybeUninit, size_of};
use std::os::unix::fs::MetadataExt;

use libc::c_int;
use rustix::process::{Pid, Signal, kill_process};

use crate::ownership::_helpers::lock::carries_claim;
use crate::ownership::constants::{CLAIM_FD, PROC_PIDFDVNODEPATHINFO};
use crate::ownership::models::{RecoveredOwnership, VnodeDescriptor};

pub(crate) fn signal_owners(owner: &RecoveredOwnership, force: bool) -> std::io::Result<()> {
    let signal = if force { Signal::KILL } else { Signal::TERM };
    let expected = owner.lock.metadata()?;
    for raw in process_ids()? {
        if !holds_descriptor(raw, &expected) {
            continue;
        }
        let Some(started) = start_time(raw) else {
            continue;
        };
        let Some(environment) = initial_environment(raw) else {
            continue;
        };
        if !carries_claim(&environment, owner.claim_id)
            || !holds_descriptor(raw, &expected)
            || start_time(raw) != Some(started)
        {
            continue;
        }
        let Some(pid) = Pid::from_raw(raw) else {
            continue;
        };
        match kill_process(pid, signal) {
            Ok(()) | Err(rustix::io::Errno::SRCH) => {}
            Err(error) => return Err(error.into()),
        }
    }
    Ok(())
}

/// Every current process ID; the list is retaken whenever the buffer may have been too small.
#[allow(unsafe_code)]
fn process_ids() -> std::io::Result<Vec<c_int>> {
    loop {
        // SAFETY: a null buffer with size zero asks libproc only for the process count.
        let count = unsafe { libc::proc_listallpids(std::ptr::null_mut(), 0) };
        let count = usize::try_from(count).map_err(|_| std::io::Error::last_os_error())?;
        let mut pids: Vec<c_int> = vec![0; count + 64];
        let bytes = c_int::try_from(pids.len() * size_of::<c_int>())
            .map_err(|_| std::io::Error::other("process list too large"))?;
        // SAFETY: `pids` is writable for exactly `bytes` bytes of `c_int` process IDs.
        let written = unsafe { libc::proc_listallpids(pids.as_mut_ptr().cast(), bytes) };
        let written = usize::try_from(written).map_err(|_| std::io::Error::last_os_error())?;
        if written < pids.len() {
            pids.truncate(written);
            return Ok(pids);
        }
    }
}

/// The process's claim descriptor currently refers to the same vnode as `expected`.
#[allow(unsafe_code)]
fn holds_descriptor(pid: c_int, expected: &std::fs::Metadata) -> bool {
    let mut info = MaybeUninit::<VnodeDescriptor>::zeroed();
    let Ok(size) = c_int::try_from(size_of::<VnodeDescriptor>()) else {
        return false;
    };
    // SAFETY: `info` is writable for one `vnode_fdinfowithpath` record of `size` bytes.
    let written = unsafe {
        libc::proc_pidfdinfo(
            pid,
            CLAIM_FD,
            PROC_PIDFDVNODEPATHINFO,
            info.as_mut_ptr().cast(),
            size,
        )
    };
    if written != size {
        return false;
    }
    // SAFETY: the record was zero-initialised and libproc filled all `size` bytes of it.
    let stat = unsafe { info.assume_init() }.vnode.vip_vi.vi_stat;
    stat.vst_ino == expected.ino() && i64::from(stat.vst_dev as i32) as u64 == expected.dev()
}

/// Seconds and microseconds of process start; a reused PID has a different start time.
#[allow(unsafe_code)]
fn start_time(pid: c_int) -> Option<(u64, u64)> {
    let mut info = MaybeUninit::<libc::proc_bsdinfo>::zeroed();
    let Ok(size) = c_int::try_from(size_of::<libc::proc_bsdinfo>()) else {
        return None;
    };
    // SAFETY: `info` is writable for one `proc_bsdinfo` record of `size` bytes.
    let written = unsafe {
        libc::proc_pidinfo(
            pid,
            libc::PROC_PIDTBSDINFO,
            0,
            info.as_mut_ptr().cast(),
            size,
        )
    };
    if written != size {
        return None;
    }
    // SAFETY: the record was zero-initialised and libproc filled all `size` bytes of it.
    let info = unsafe { info.assume_init() };
    Some((info.pbi_start_tvsec, info.pbi_start_tvusec))
}

/// The NUL-separated environment the process was executed with, from `KERN_PROCARGS2`.
#[allow(unsafe_code)]
fn initial_environment(pid: c_int) -> Option<Vec<u8>> {
    let mut maximum: c_int = 0;
    let mut length = size_of::<c_int>();
    let mut name = [libc::CTL_KERN, libc::KERN_ARGMAX];
    // SAFETY: `maximum` is writable for `length` bytes; no new value is set.
    let status = unsafe {
        libc::sysctl(
            name.as_mut_ptr(),
            2,
            (&raw mut maximum).cast(),
            &raw mut length,
            std::ptr::null_mut(),
            0,
        )
    };
    if status != 0 {
        return None;
    }
    let Ok(maximum) = usize::try_from(maximum) else {
        return None;
    };
    let mut buffer = vec![0_u8; maximum];
    let mut length = buffer.len();
    let mut name = [libc::CTL_KERN, libc::KERN_PROCARGS2, pid];
    // SAFETY: `buffer` is writable for `length` bytes; no new value is set.
    let status = unsafe {
        libc::sysctl(
            name.as_mut_ptr(),
            3,
            buffer.as_mut_ptr().cast(),
            &raw mut length,
            std::ptr::null_mut(),
            0,
        )
    };
    if status != 0 {
        return None;
    }
    buffer.truncate(length);
    environment_block(&buffer).map(<[u8]>::to_vec)
}

/// `KERN_PROCARGS2` holds `argc`, the executable path, NUL padding, the arguments, then the environment.
fn environment_block(arguments: &[u8]) -> Option<&[u8]> {
    let (count, mut rest) = arguments.split_first_chunk::<4>()?;
    let count = u32::from_ne_bytes(*count);
    rest = rest.get(rest.iter().position(|byte| *byte == 0)?..)?;
    rest = rest.get(rest.iter().position(|byte| *byte != 0)?..)?;
    for _ in 0..count {
        rest = rest.get(rest.iter().position(|byte| *byte == 0)? + 1..)?;
    }
    Some(rest)
}
