//! A saved person session asks a location to republish; the server checks the permission.

use std::path::Path;

use sqlbuild_orchestration::platform::models::{PersonWorkRequest, RepublishStatus};
use uuid::Uuid;

use crate::person_login::_helpers::transport;

pub(crate) fn request_republish(
    server: (&str, Option<&Path>),
    (location, target): (Uuid, &str),
    request: &PersonWorkRequest,
) -> Result<RepublishStatus, std::io::Error> {
    transport::with_person_session(server, async |client, credential| {
        transport::republish(client, credential, (location, target), request).await
    })
}
