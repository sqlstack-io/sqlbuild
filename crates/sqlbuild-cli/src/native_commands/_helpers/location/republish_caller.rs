//! Who `sqb location republish` calls as: an API key over any transport, or a saved HTTPS session.

use std::io::Error;

use sqlbuild_orchestration::platform::main::work::call_remote_republish::call_remote_republish;
use sqlbuild_orchestration::platform::models::{RepublishCall, RepublishStatus};
use uuid::Uuid;

use crate::native_commands::constants::API_KEY_VARIABLE;
use crate::native_commands::models::RepublishCaller;
use crate::person_login::main::call_republish::call_republish;

/// The environment's API key when there is one, otherwise the saved `sqb login` session.
pub(crate) fn republish_caller(
    server: String,
    ca: Option<std::path::PathBuf>,
) -> Result<RepublishCaller, Error> {
    let Some(key) = std::env::var_os(API_KEY_VARIABLE) else {
        return Ok(RepublishCaller::Session { server, ca });
    };
    let key: String = key
        .into_string()
        .map_err(|_| Error::other(format!("{API_KEY_VARIABLE} must be UTF-8")))?;
    if key.trim().is_empty() {
        return Err(Error::other(format!(
            "{API_KEY_VARIABLE} must not be blank"
        )));
    }
    if ca.is_some() {
        return Err(Error::other(format!(
            "--ca-cert applies only to a sqb login session, and {API_KEY_VARIABLE} is set"
        )));
    }
    Ok(RepublishCaller::Key { server, key })
}

/// The republish after `call`, made as `caller`.
pub(crate) fn call_as(
    caller: &RepublishCaller,
    scope: (Uuid, &str),
    call: RepublishCall<'_>,
) -> Result<RepublishStatus, Error> {
    let status: Option<RepublishStatus> = match caller {
        RepublishCaller::Key { server, key } => tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?
            .block_on(call_remote_republish((server, key), scope, call))
            .map_err(Error::other)?,
        RepublishCaller::Session { server, ca } => {
            call_republish((server, ca.as_deref()), scope, call)?
        }
    };
    status.ok_or_else(|| Error::other("the server has no such republish request"))
}
