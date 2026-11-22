//! A saved person session asks for a republish or reads one back; the server checks the permission.

use std::path::Path;

use sqlbuild_orchestration::platform::models::{RepublishCall, RepublishStatus};
use uuid::Uuid;

use crate::person_login::_helpers::transport;

pub(crate) fn call_republish(
    server: (&str, Option<&Path>),
    scope: (Uuid, &str),
    call: RepublishCall<'_>,
) -> Result<Option<RepublishStatus>, std::io::Error> {
    transport::with_person_session(server, async |client, credential| {
        transport::republish(client, credential, scope, call).await
    })
}
