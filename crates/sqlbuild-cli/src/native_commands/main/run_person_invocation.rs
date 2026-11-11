//! Person control commands use the saved HTTPS session, never privileged database access.

use crate::argv::models::{Namespace, Stream};
use crate::native_commands::_helpers::decision::text;
use crate::native_commands::_helpers::terminal::write_output;
use crate::native_commands::main::_run_warehouse_sync::run_warehouse_sync;
use crate::person_login::main::read_node_sql::read_node_sql;

pub(crate) fn run_person_invocation(namespace: &Namespace) -> Option<i32> {
    if text(namespace, "command").as_deref() != Some("control") {
        return None;
    }
    if text(namespace, "person_action").as_deref() == Some("sync") {
        return Some(run_warehouse_sync(namespace));
    }
    drop(write_output(&[(
        Stream::Stderr,
        "Reading published SQL…\n",
    )]));
    let result = (|| {
        let location = text(namespace, "person_location")
            .unwrap_or_default()
            .parse()
            .map_err(|_| std::io::Error::other("--location-id must be a UUID"))?;
        let ca = text(namespace, "login_ca").map(std::path::PathBuf::from);
        let sql = read_node_sql(
            (
                &text(namespace, "login_server").unwrap_or_default(),
                ca.as_deref(),
            ),
            (
                location,
                &text(namespace, "target").unwrap_or_default(),
                &text(namespace, "person_node").unwrap_or_default(),
            ),
        )?;
        serde_json::to_string(&sql).map_err(std::io::Error::other)
    })();
    Some(match result {
        Ok(json) => {
            let lost = write_output(&[(Stream::Stdout, &format!("{json}\n"))]);
            if lost.is_some() {
                drop(write_output(&[(
                    Stream::Stderr,
                    "SQL inspection failed: could not write JSON to stdout\n",
                )]));
                1
            } else {
                drop(write_output(&[(
                    Stream::Stderr,
                    "Published SQL read successfully\n",
                )]));
                0
            }
        }
        Err(error) => {
            drop(write_output(&[(
                Stream::Stderr,
                &format!("SQL inspection failed: {error}\n"),
            )]));
            1
        }
    })
}
