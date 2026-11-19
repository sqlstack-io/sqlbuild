//! The fingerprint metadata that participates in version identity.

use crate::planner_changes::models::IdentityPayload;
use sqlbuild_core::fingerprint_identity::main::decoded_object::decoded_object;
use sqlbuild_core::fingerprint_identity::main::normalize_version_identity_metadata::normalize_version_identity_metadata;
use sqlbuild_core::fingerprint_identity::main::without_default_refresh_mode::without_default_refresh_mode;

/// The metadata without audit-gate, migration, declared-column and default refresh-mode entries; with `without_functions`, also without function hashes.
pub fn version_identity_payload(
    metadata_json: Option<&str>,
    without_functions: bool,
) -> IdentityPayload {
    let Some(text) = metadata_json else {
        return IdentityPayload::Absent;
    };
    match decoded_object(text) {
        Err(_) => IdentityPayload::Raw(text.to_owned()),
        Ok(None) => IdentityPayload::Absent,
        Ok(Some(mut object)) => {
            normalize_version_identity_metadata(&mut object, without_functions);
            IdentityPayload::Object(without_default_refresh_mode(object))
        }
    }
}
