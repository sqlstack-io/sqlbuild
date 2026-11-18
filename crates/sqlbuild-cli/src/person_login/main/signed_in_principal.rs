//! The principal a person is signed in as with `sqb login`, read from local credentials only.

use std::path::Path;

use chrono::Utc;
use sqlbuild_orchestration::api::main::parse_origin::parse_origin;

use crate::person_login::_helpers::build_sync::configured_location;
use crate::person_login::_helpers::credentials;

/// `principal:<id>` of an unexpired session for the configured server; None when not signed in.
pub fn signed_in_principal(project_dir: &Path) -> Result<Option<String>, std::io::Error> {
    let Some((server, _)) = configured_location(project_dir)? else {
        return Ok(None);
    };
    let origin: String = parse_origin(&server, "CLI").map_err(std::io::Error::other)?;
    let Some(credential) = credentials::load(&origin)? else {
        return Ok(None);
    };
    Ok((credential.identity.expires_at > Utc::now())
        .then(|| format!("principal:{}", credential.identity.principal_id)))
}
