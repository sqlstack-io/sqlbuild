//! A location supervisor stays native while each compilation uses the installed project interpreter.

use std::path::Path;

use crate::argv::models::{Namespace, Stream};
use crate::native_commands::_helpers::decision::text;
use crate::native_commands::_helpers::execution::location_options;
use crate::native_commands::_helpers::terminal::write_output;
use crate::native_commands::main::_run_location::run_location;

pub(crate) fn run_location_invocation(namespace: &Namespace, script: &Path) -> Option<i32> {
    if text(namespace, "command").as_deref() != Some("location")
        || text(namespace, "location_command").as_deref() != Some("serve")
    {
        return None;
    }
    let result = (|| {
        let options = location_options(namespace)?;
        let bearer = std::env::var("SQLBUILD_LOCATION_TOKEN").map_err(|_| {
            std::io::Error::other(
                "SQLBUILD_LOCATION_TOKEN must contain a UTF-8 location credential",
            )
        })?;
        if bearer.trim().is_empty() {
            return Err(std::io::Error::other("SQLBUILD_LOCATION_TOKEN must not be blank").into());
        }
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()?;
        runtime.block_on(run_location(options, script, &bearer))
    })();
    Some(match result {
        Ok(()) => {
            drop(write_output(&[(
                Stream::Stderr,
                "Code location stopped successfully\n",
            )]));
            0
        }
        Err(error) => {
            drop(write_output(&[(
                Stream::Stderr,
                &format!("Code location failed: {error}\n"),
            )]));
            1
        }
    })
}
