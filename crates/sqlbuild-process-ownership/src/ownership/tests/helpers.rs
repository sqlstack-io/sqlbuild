use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::Duration;

use uuid::Uuid;

use crate::ownership::main::prepare_worker::prepare_worker;
use crate::ownership::main::start_worker::start_worker;
use crate::ownership::main::worker_exited::worker_exited;
use crate::ownership::models::Claim;

pub(super) fn claims(project_dir: &Path) -> [Claim<'_>; 2] {
    let location_id = Uuid::now_v7();
    [Uuid::now_v7(), Uuid::now_v7()].map(|claim_id| Claim {
        project_dir,
        location_id,
        claim_id,
    })
}

#[cfg(unix)]
fn sleeper(seconds: u32) -> Command {
    let mut command = Command::new("sh");
    command.args(["-c", &format!("sleep {seconds}")]);
    command
}

#[cfg(windows)]
fn sleeper(seconds: u32) -> Command {
    let mut command = Command::new("cmd");
    command.args(["/c", &format!("ping -n {} 127.0.0.1", seconds + 1)]);
    command
}

/// The worker is owned by `claims[holder]`, if any, and carries `claims[marker]`'s ID.
pub(super) fn spawn(
    claims: &[Claim<'_>; 2],
    (holder, marker): (Option<usize>, usize),
    seconds: u32,
) -> Child {
    let mut command = sleeper(seconds);
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let (mut command, pending) = holder.map(|index| &claims[index]).into_iter().fold(
        (command, None),
        |(command, _), claim| {
            let (command, pending) = prepare_worker(command, claim).expect("prepare ownership");
            (command, Some(pending))
        },
    );
    command.env(
        crate::ownership::constants::WORKER_CLAIM_ENV,
        claims[marker].claim_id.to_string(),
    );
    let child = command.spawn().expect("spawn worker");
    pending
        .map(|pending| start_worker(pending, child.id()))
        .transpose()
        .expect("start owned worker");
    child
}

pub(super) fn exited_within(child: &mut Child, within: Duration) -> bool {
    let attempts = within.as_millis() / 20 + 1;
    (0..attempts).any(|_| {
        child.try_wait().expect("observe worker").is_some() || {
            std::thread::sleep(Duration::from_millis(20));
            false
        }
    })
}

pub(super) fn recovered_exited_within(
    owner: &crate::ownership::models::RecoveredOwnership,
    within: Duration,
) -> bool {
    let attempts = within.as_millis() / 20 + 1;
    (0..attempts).any(|_| {
        worker_exited(owner).expect("observe recovered worker") || {
            std::thread::sleep(Duration::from_millis(20));
            false
        }
    })
}

pub(super) fn reap(mut child: Child) {
    drop(child.kill());
    child.wait().expect("reap worker");
}

#[cfg(windows)]
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};

#[cfg(windows)]
use windows_sys::Win32::Foundation::HANDLE;
#[cfg(windows)]
use windows_sys::Win32::Security::{
    CreateRestrictedToken, DISABLE_MAX_PRIVILEGE, ImpersonateLoggedOnUser, RevertToSelf,
    SID_AND_ATTRIBUTES, TOKEN_DUPLICATE, TOKEN_IMPERSONATE, TOKEN_QUERY,
};
#[cfg(windows)]
use windows_sys::Win32::System::JobObjects::OpenJobObjectW;
#[cfg(windows)]
use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

#[cfg(windows)]
use crate::ownership::_helpers::windows::{account, account_sid};

/// `Ok(())` when the named job opens with `access`, otherwise the Win32 error code.
#[cfg(windows)]
#[allow(unsafe_code)]
pub(super) fn open_job(name: &[u16], access: u32) -> Result<(), u32> {
    // SAFETY: `name` is NUL-terminated and outlives the call.
    let job = unsafe { OpenJobObjectW(access, 0, name.as_ptr()) };
    let error = std::io::Error::last_os_error()
        .raw_os_error()
        .map_or(0, i32::cast_unsigned);
    (!job.is_null())
        // SAFETY: a valid handle that nothing else closes.
        .then(|| drop(unsafe { OwnedHandle::from_raw_handle(job) }))
        .ok_or(error)
}

/// Same session, same user, but with the user SID deny-only and no privileges.
#[cfg(windows)]
#[allow(unsafe_code)]
fn foreign_token() -> OwnedHandle {
    let mut token: HANDLE = std::ptr::null_mut();
    // SAFETY: opens this process's own token.
    let opened = unsafe {
        OpenProcessToken(
            GetCurrentProcess(),
            TOKEN_DUPLICATE | TOKEN_QUERY | TOKEN_IMPERSONATE,
            &raw mut token,
        )
    };
    assert_ne!(opened, 0, "{}", std::io::Error::last_os_error());
    // SAFETY: a valid handle that nothing else closes.
    let token = unsafe { OwnedHandle::from_raw_handle(token) };
    let user = account().expect("account");
    let disabled = SID_AND_ATTRIBUTES {
        Sid: account_sid(&user),
        Attributes: 0,
    };
    let mut restricted: HANDLE = std::ptr::null_mut();
    // SAFETY: the SID lives in `user`; `restricted` receives a new token handle.
    let created = unsafe {
        CreateRestrictedToken(
            token.as_raw_handle(),
            DISABLE_MAX_PRIVILEGE,
            1,
            &raw const disabled,
            0,
            std::ptr::null(),
            0,
            std::ptr::null(),
            &raw mut restricted,
        )
    };
    assert_ne!(created, 0, "{}", std::io::Error::last_os_error());
    // SAFETY: a valid handle that nothing else closes.
    unsafe { OwnedHandle::from_raw_handle(restricted) }
}

/// Run `attempt` on a thread that impersonates a foreign identity for its whole duration.
#[cfg(windows)]
#[allow(unsafe_code)]
pub(super) fn as_foreign_identity<T: Send>(attempt: impl FnOnce() -> T + Send) -> T {
    std::thread::scope(|scope| {
        scope
            .spawn(|| {
                let token = foreign_token();
                // SAFETY: impersonation applies to this scoped thread only and is reverted below.
                let impersonated = unsafe { ImpersonateLoggedOnUser(token.as_raw_handle()) };
                assert_ne!(impersonated, 0, "{}", std::io::Error::last_os_error());
                let result = attempt();
                // SAFETY: ends this thread's impersonation.
                assert_ne!(unsafe { RevertToSelf() }, 0);
                result
            })
            .join()
            .expect("foreign identity thread")
    })
}
