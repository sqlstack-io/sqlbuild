//! Forward one diagnostic's JSON to `session` for `invocation_id`; unparsable ones are skipped.

use sqlbuild_recall::writer::models::RecallSession;

/// Forward one diagnostic's JSON to `session` for `invocation_id`; unparsable ones are skipped.
pub fn forward_diagnostic(
    session: Option<&RecallSession>,
    invocation_id: &str,
    diagnostic_json: &str,
) {
    if let Some(session) = session.filter(|session| session.invocation_id() == invocation_id) {
        drop(session.publish_diagnostic(diagnostic_json));
    }
}
