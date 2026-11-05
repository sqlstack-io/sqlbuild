//! Windows: each claim's worker runs in a private named job owned by, and open only to, this account.

use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};

use windows_sys::Win32::Foundation::{
    DuplicateHandle, ERROR_ALREADY_EXISTS, ERROR_FILE_NOT_FOUND, GetLastError, HANDLE,
    INVALID_HANDLE_VALUE, LocalFree,
};
use windows_sys::Win32::Security::Authorization::{
    ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW, GetSecurityInfo,
    SDDL_REVISION_1, SE_KERNEL_OBJECT,
};
use windows_sys::Win32::Security::{
    EqualSid, GetTokenInformation, OWNER_SECURITY_INFORMATION, PSECURITY_DESCRIPTOR, PSID,
    SECURITY_ATTRIBUTES, TOKEN_QUERY, TOKEN_USER, TokenUser,
};
use windows_sys::Win32::Storage::FileSystem::READ_CONTROL;
use windows_sys::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, TH32CS_SNAPTHREAD, THREADENTRY32, Thread32First, Thread32Next,
};
use windows_sys::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JOBOBJECT_BASIC_ACCOUNTING_INFORMATION,
    JobObjectBasicAccountingInformation, OpenJobObjectW, QueryInformationJobObject,
    TerminateJobObject,
};
use windows_sys::Win32::System::SystemServices::{JOB_OBJECT_QUERY, JOB_OBJECT_TERMINATE};
use windows_sys::Win32::System::Threading::{
    GetCurrentProcess, OpenProcess, OpenProcessToken, OpenThread, PROCESS_DUP_HANDLE,
    PROCESS_SET_QUOTA, PROCESS_TERMINATE, ResumeThread, THREAD_SUSPEND_RESUME,
};

use crate::ownership::constants::JOB_PREFIX;
use crate::ownership::models::Claim;

pub(crate) fn job_name(claim: &Claim<'_>) -> Vec<u16> {
    format!("{JOB_PREFIX}{}-{}", claim.location_id, claim.claim_id)
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect()
}

/// Take ownership of a handle a successful Win32 call just returned.
#[allow(unsafe_code)]
fn owned(handle: HANDLE) -> OwnedHandle {
    // SAFETY: callers pass a valid, open handle that nothing else closes.
    unsafe { OwnedHandle::from_raw_handle(handle) }
}

/// The `TOKEN_USER` record of this process; its SID points into the returned buffer.
#[allow(unsafe_code)]
pub(crate) fn account() -> std::io::Result<Vec<u8>> {
    let mut token: HANDLE = std::ptr::null_mut();
    // SAFETY: `token` receives a new query handle to this process's own token.
    if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &raw mut token) } == 0 {
        return Err(std::io::Error::last_os_error());
    }
    let token = owned(token);
    let mut length = 0_u32;
    // SAFETY: a null buffer of size zero asks only for the required length.
    unsafe {
        GetTokenInformation(
            token.as_raw_handle(),
            TokenUser,
            std::ptr::null_mut(),
            0,
            &raw mut length,
        )
    };
    let mut buffer = vec![
        0_u8;
        usize::try_from(length)
            .unwrap_or(0)
            .max(size_of::<TOKEN_USER>())
    ];
    let capacity = u32::try_from(buffer.len()).map_err(std::io::Error::other)?;
    // SAFETY: `buffer` is writable for `capacity` bytes.
    let read = unsafe {
        GetTokenInformation(
            token.as_raw_handle(),
            TokenUser,
            buffer.as_mut_ptr().cast(),
            capacity,
            &raw mut length,
        )
    };
    if read == 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(buffer)
}

#[allow(unsafe_code)]
pub(crate) fn account_sid(account: &[u8]) -> PSID {
    // SAFETY: `account` holds a complete `TOKEN_USER` written by `GetTokenInformation`.
    unsafe { std::ptr::read_unaligned(account.as_ptr().cast::<TOKEN_USER>()) }
        .User
        .Sid
}

#[allow(unsafe_code)]
fn sid_text(sid: PSID) -> std::io::Result<String> {
    let mut text: windows_sys::core::PWSTR = std::ptr::null_mut();
    // SAFETY: `sid` is valid; `text` receives a LocalAlloc string freed below.
    if unsafe { ConvertSidToStringSidW(sid, &raw mut text) } == 0 {
        return Err(std::io::Error::last_os_error());
    }
    // SAFETY: `text` is a NUL-terminated wide string; its length is found before it is read.
    let units = unsafe {
        let length = (0..).take_while(|&index| *text.add(index) != 0).count();
        std::slice::from_raw_parts(text, length)
    };
    let sid = String::from_utf16_lossy(units);
    // SAFETY: `text` came from `ConvertSidToStringSidW` and is not used again.
    unsafe { LocalFree(text.cast()) };
    Ok(sid)
}

/// The job's owner and only access entry are this process's account; an existing name is refused.
#[allow(unsafe_code)]
pub(crate) fn create_job(claim: &Claim<'_>) -> std::io::Result<OwnedHandle> {
    let account = account()?;
    let sid = sid_text(account_sid(&account))?;
    let descriptor_text: Vec<u16> = format!("O:{sid}D:P(A;;GA;;;{sid})")
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    let mut descriptor: PSECURITY_DESCRIPTOR = std::ptr::null_mut();
    // SAFETY: `descriptor` receives a LocalAlloc security descriptor freed below.
    let converted = unsafe {
        ConvertStringSecurityDescriptorToSecurityDescriptorW(
            descriptor_text.as_ptr(),
            SDDL_REVISION_1,
            &raw mut descriptor,
            std::ptr::null_mut(),
        )
    };
    if converted == 0 {
        return Err(std::io::Error::last_os_error());
    }
    let attributes = SECURITY_ATTRIBUTES {
        nLength: u32::try_from(size_of::<SECURITY_ATTRIBUTES>()).unwrap_or(0),
        lpSecurityDescriptor: descriptor,
        bInheritHandle: 0,
    };
    let name = job_name(claim);
    // SAFETY: `attributes` and `name` outlive the call.
    let job = unsafe { CreateJobObjectW(&raw const attributes, name.as_ptr()) };
    // SAFETY: reads this thread's last error before any other call can change it.
    let error = unsafe { GetLastError() };
    // SAFETY: `descriptor` came from the conversion above and is not used again.
    unsafe { LocalFree(descriptor) };
    if job.is_null() {
        return Err(std::io::Error::from_raw_os_error(error.cast_signed()));
    }
    let job = owned(job);
    if error == ERROR_ALREADY_EXISTS {
        return Err(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            "a worker job with this claim's name already exists",
        ));
    }
    Ok(job)
}

#[allow(unsafe_code)]
pub(crate) fn open_job(claim: &Claim<'_>) -> std::io::Result<Option<OwnedHandle>> {
    let name = job_name(claim);
    // SAFETY: `name` outlives the call.
    let job = unsafe {
        OpenJobObjectW(
            JOB_OBJECT_QUERY | JOB_OBJECT_TERMINATE | READ_CONTROL,
            0,
            name.as_ptr(),
        )
    };
    if job.is_null() {
        let error = std::io::Error::last_os_error();
        return match error.raw_os_error() {
            Some(code) if code.cast_unsigned() == ERROR_FILE_NOT_FOUND => Ok(None),
            _ => Err(error),
        };
    }
    let job = owned(job);
    Ok(owned_by_this_account(&job)?.then_some(job))
}

#[allow(unsafe_code)]
fn owned_by_this_account(job: &OwnedHandle) -> std::io::Result<bool> {
    let account = account()?;
    let mut owner: PSID = std::ptr::null_mut();
    let mut descriptor: PSECURITY_DESCRIPTOR = std::ptr::null_mut();
    // SAFETY: the handle has READ_CONTROL; `descriptor` is LocalAlloc'd and freed below.
    let status = unsafe {
        GetSecurityInfo(
            job.as_raw_handle(),
            SE_KERNEL_OBJECT,
            OWNER_SECURITY_INFORMATION,
            &raw mut owner,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            &raw mut descriptor,
        )
    };
    if status != 0 {
        return Err(std::io::Error::from_raw_os_error(status.cast_signed()));
    }
    // SAFETY: `owner` points into `descriptor`, which is still allocated; `account` holds our SID.
    let same = unsafe { EqualSid(owner, account_sid(&account)) } != 0;
    // SAFETY: `descriptor` came from `GetSecurityInfo` and `owner` is not used again.
    unsafe { LocalFree(descriptor) };
    Ok(same)
}

#[allow(unsafe_code)]
pub(crate) fn active_processes(job: &OwnedHandle) -> std::io::Result<u32> {
    let mut info = JOBOBJECT_BASIC_ACCOUNTING_INFORMATION::default();
    let size = u32::try_from(size_of::<JOBOBJECT_BASIC_ACCOUNTING_INFORMATION>())
        .map_err(std::io::Error::other)?;
    // SAFETY: `info` is writable for one accounting record of `size` bytes.
    let queried = unsafe {
        QueryInformationJobObject(
            job.as_raw_handle(),
            JobObjectBasicAccountingInformation,
            (&raw mut info).cast(),
            size,
            std::ptr::null_mut(),
        )
    };
    if queried == 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(info.ActiveProcesses)
}

#[allow(unsafe_code)]
pub(crate) fn terminate(job: &OwnedHandle) -> std::io::Result<()> {
    // SAFETY: the handle is open with terminate access to a job this account owns.
    if unsafe { TerminateJobObject(job.as_raw_handle(), 1) } == 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(())
}

/// Put the suspended child in the job, give it a job handle, then let it run.
#[allow(unsafe_code)]
pub(crate) fn adopt(job: &OwnedHandle, process_id: u32) -> std::io::Result<()> {
    // SAFETY: opens our own unreaped, suspended child, so its ID cannot have been reused.
    let process = unsafe {
        OpenProcess(
            PROCESS_SET_QUOTA | PROCESS_TERMINATE | PROCESS_DUP_HANDLE,
            0,
            process_id,
        )
    };
    if process.is_null() {
        return Err(std::io::Error::last_os_error());
    }
    let process = owned(process);
    // SAFETY: both handles are open; the child is suspended and has run no code.
    if unsafe { AssignProcessToJobObject(job.as_raw_handle(), process.as_raw_handle()) } == 0 {
        return Err(std::io::Error::last_os_error());
    }
    let mut copy: HANDLE = std::ptr::null_mut();
    // SAFETY: duplicates our job handle into the child, which keeps the named job alive until it exits.
    let duplicated = unsafe {
        DuplicateHandle(
            GetCurrentProcess(),
            job.as_raw_handle(),
            process.as_raw_handle(),
            &raw mut copy,
            JOB_OBJECT_QUERY,
            0,
            0,
        )
    };
    if duplicated == 0 {
        return Err(std::io::Error::last_os_error());
    }
    resume(process_id)
}

#[allow(unsafe_code)]
fn resume(process_id: u32) -> std::io::Result<()> {
    // SAFETY: takes a snapshot of all threads; the handle is closed when `snapshot` drops.
    let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0) };
    if snapshot == INVALID_HANDLE_VALUE {
        return Err(std::io::Error::last_os_error());
    }
    let snapshot = owned(snapshot);
    let mut entry = THREADENTRY32 {
        dwSize: u32::try_from(size_of::<THREADENTRY32>()).unwrap_or(0),
        ..Default::default()
    };
    let mut resumed = 0_usize;
    // SAFETY: `entry` is a correctly sized record for this snapshot.
    let mut more = unsafe { Thread32First(snapshot.as_raw_handle(), &raw mut entry) } != 0;
    while more {
        if entry.th32OwnerProcessID == process_id {
            // SAFETY: opens a thread of our own suspended child.
            let thread = unsafe { OpenThread(THREAD_SUSPEND_RESUME, 0, entry.th32ThreadID) };
            if thread.is_null() {
                return Err(std::io::Error::last_os_error());
            }
            let thread = owned(thread);
            // SAFETY: the thread handle has suspend/resume access.
            if unsafe { ResumeThread(thread.as_raw_handle()) } == u32::MAX {
                return Err(std::io::Error::last_os_error());
            }
            resumed += 1;
        }
        // SAFETY: `entry` is a correctly sized record for this snapshot.
        more = unsafe { Thread32Next(snapshot.as_raw_handle(), &raw mut entry) } != 0;
    }
    if resumed == 0 {
        return Err(std::io::Error::other("worker has no thread to resume"));
    }
    Ok(())
}
