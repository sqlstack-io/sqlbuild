//! Close the session within its shutdown budget; a further interrupt stops waiting at once. Notices end with the command's own outcome.

use crate::recall::_helpers::session::closing_notices;
use crate::recall::_helpers::session::notify;
use crate::recall::_helpers::session::prefixed;
use crate::recall::constants::CLOSE_POLL_MS;
use crate::recall::constants::RECALL_HISTORY;
use crate::recall::models::InvocationRecall;
use pyo3::prelude::{PyResult, Python};
use sqlbuild_cli::dispatch::main::closing_status::closing_status;
use std::time::Duration;

/// Close the session within its shutdown budget; a further interrupt stops waiting at once. Notices end with the command's own outcome.
pub fn finish_invocation_recall(
    py: Python<'_>,
    recall: &InvocationRecall,
    exit_code: Option<i64>,
    sqlbuild_version: &str,
) -> PyResult<()> {
    let Some(session) = &recall.session else {
        return Ok(());
    };
    session.begin_close();
    while !py.detach(|| session.wait_closed(Duration::from_millis(CLOSE_POLL_MS))) {
        if let Err(interrupt) = py.check_signals() {
            session.abandon();
            return Err(interrupt);
        }
    }
    let information: Vec<String> = session
        .information()
        .iter()
        .map(|text| prefixed(text))
        .collect();
    let notices: Vec<String> = closing_notices(recall, session, sqlbuild_version);
    if !information.is_empty() || !notices.is_empty() {
        let affected: Option<&str> = (!notices.is_empty()).then_some(RECALL_HISTORY);
        notify(
            py,
            Some(
                information.concat()
                    + &notices.concat()
                    + &closing_status(&recall.command, exit_code, affected),
            ),
        );
    }
    Ok(())
}
