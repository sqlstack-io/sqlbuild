//! A saved person session can inspect published SQL without opening a project or warehouse.

use std::path::Path;

use sqlbuild_orchestration::api::main::parse_origin::parse_origin;
use sqlbuild_orchestration::snapshots::models::NodeSql;
use uuid::Uuid;

use crate::person_login::_helpers::{credentials, transport};

pub(crate) fn read_node_sql(
    (server, ca): (&str, Option<&Path>),
    (location, target, node): (Uuid, &str, &str),
) -> Result<Option<NodeSql>, std::io::Error> {
    let origin = parse_origin(server, "CLI").map_err(std::io::Error::other)?;
    if target.trim().is_empty() || node.trim().is_empty() {
        return Err(std::io::Error::other(
            "target and exact node key must not be blank",
        ));
    }
    let credential = credentials::load(&origin)?
        .ok_or_else(|| std::io::Error::other("not signed in; run sqb login"))?;
    let client = transport::connect(&origin, ca)?;
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    runtime.block_on(transport::node_sql(
        &client,
        &credential,
        (location, target, node),
    ))
}
