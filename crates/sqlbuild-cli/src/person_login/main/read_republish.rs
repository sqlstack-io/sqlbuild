//! A saved person session reads a republish back.

use std::path::Path;

use sqlbuild_orchestration::platform::models::RepublishStatus;
use uuid::Uuid;

use crate::person_login::_helpers::transport;

pub(crate) fn read_republish(
    server: (&str, Option<&Path>),
    (location, target, request_id): (Uuid, &str, Uuid),
    wait: u64,
) -> Result<Option<RepublishStatus>, std::io::Error> {
    transport::with_person_session(server, async |client, credential| {
        transport::republish_status(client, credential, (location, target, request_id), wait).await
    })
}
