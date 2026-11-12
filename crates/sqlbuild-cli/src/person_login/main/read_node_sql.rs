//! A saved person session can inspect published SQL without opening a project or warehouse.

use std::path::Path;

use sqlbuild_orchestration::snapshots::models::NodeSql;
use uuid::Uuid;

use crate::person_login::_helpers::transport;

pub(crate) fn read_node_sql(
    server: (&str, Option<&Path>),
    (location, target, node): (Uuid, &str, &str),
) -> Result<Option<NodeSql>, std::io::Error> {
    if target.trim().is_empty() || node.trim().is_empty() {
        return Err(std::io::Error::other(
            "target and exact node key must not be blank",
        ));
    }
    transport::with_person_session(server, async |client, credential| {
        transport::node_sql(client, credential, (location, target, node)).await
    })
}
