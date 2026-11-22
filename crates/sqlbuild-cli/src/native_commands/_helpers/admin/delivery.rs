//! Deliver an admin result: a secret, rows or JSON on stdout, then the line saying what was done on stderr.

use crate::argv::models::Stream;
use crate::native_commands::_helpers::admin::results::result_json;
use crate::native_commands::_helpers::admin::rows::stdout_text;
use crate::native_commands::_helpers::admin::summaries::summary_line;
use crate::native_commands::_helpers::command::terminal::write_output;
use crate::native_commands::constants::ADMIN_DELIVERY_FAILED;
use crate::native_commands::models::AdminDone;

/// Print `done` and return the exit status; the line follows only output that arrived.
pub(crate) fn deliver_admin_result(done: &AdminDone, json: bool) -> i32 {
    let (stdout, summary): (String, Option<String>) = if json {
        (format!("{}\n", result_json(done)), None)
    } else {
        (stdout_text(done), summary_line(done))
    };
    if write_output(&[(Stream::Stdout, &stdout)]).is_some() {
        drop(write_output(&[(Stream::Stderr, ADMIN_DELIVERY_FAILED)]));
        return 1;
    }
    if let Some(summary) = summary {
        drop(write_output(&[(Stream::Stderr, &format!("{summary}\n"))]));
    }
    0
}
