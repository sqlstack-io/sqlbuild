//! `sqb control sync` asks the location to read the warehouse; the location runs the sync itself.

use crate::argv::models::{Namespace, Stream};
use crate::native_commands::_helpers::decision::text;
use crate::native_commands::_helpers::terminal::write_output;
use crate::person_login::main::request_warehouse_sync::request_warehouse_sync;

pub(super) fn run_warehouse_sync(namespace: &Namespace) -> i32 {
    drop(write_output(&[(
        Stream::Stderr,
        "Requesting a warehouse sync…\n",
    )]));
    let result = text(namespace, "person_location")
        .unwrap_or_default()
        .parse()
        .map_err(|_| std::io::Error::other("--location-id must be a UUID"))
        .and_then(|location| {
            let ca = text(namespace, "login_ca").map(std::path::PathBuf::from);
            request_warehouse_sync(
                (
                    &text(namespace, "login_server").unwrap_or_default(),
                    ca.as_deref(),
                ),
                (location, &text(namespace, "target").unwrap_or_default()),
            )
        });
    match result {
        Ok(applied) => {
            drop(write_output(&[
                (Stream::Stdout, &format!("{{\"applied\": {applied}}}\n")),
                (
                    Stream::Stderr,
                    "Warehouse sync requested; the location starts it within a few seconds.\n",
                ),
            ]));
            0
        }
        Err(error) => {
            drop(write_output(&[(
                Stream::Stderr,
                &format!("Warehouse sync request failed: {error}\n"),
            )]));
            1
        }
    }
}
