//! A saved person session reads past runs' results for a command's run selectors; reading changes nothing.

use std::path::Path;

use sqlbuild_orchestration::runs::models::PastRunsRead;
use uuid::Uuid;

use crate::person_login::_helpers::transport;

/// What each run `ids` names did in the location and target, read with the saved session for `server`.
pub(crate) fn read_past_runs(
    server: (&str, Option<&Path>),
    (location, target): (Uuid, &str),
    ids: &[String],
) -> Result<PastRunsRead, std::io::Error> {
    transport::with_person_session(server, async |client, credential| {
        transport::past_runs(client, credential, (location, target), ids).await
    })
}
