//! A saved person session asks a location to read the warehouse now; the server checks the target.

use std::path::Path;

use uuid::Uuid;

use crate::person_login::_helpers::transport;

pub(crate) fn request_warehouse_sync(
    server: (&str, Option<&Path>),
    (location, target): (Uuid, &str),
) -> Result<bool, std::io::Error> {
    transport::with_person_session(server, async |client, credential| {
        transport::warehouse_sync(client, credential, (location, target)).await
    })
}
