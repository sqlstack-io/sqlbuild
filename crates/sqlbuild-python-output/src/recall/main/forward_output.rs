//! Forward bytes a process stream accepted to `session` when it records `invocation_id`.

use crate::recall::_helpers::replacements::in_replacement_sink;
use crate::recall::constants::STDOUT_STREAM_INDEX;
use sqlbuild_recall::records::models::OutputStream;
use sqlbuild_recall::writer::models::RecallSession;

/// Forward bytes a process stream accepted to `session` when it records `invocation_id`.
pub fn forward_output(
    session: Option<&RecallSession>,
    invocation_id: &str,
    (stream, data): (usize, &[u8]),
) {
    let Some(session) = session.filter(|session| session.invocation_id() == invocation_id) else {
        return;
    };
    if in_replacement_sink() {
        return;
    }
    let stream = if stream == STDOUT_STREAM_INDEX {
        OutputStream::Stdout
    } else {
        OutputStream::Stderr
    };
    session.append_output(stream, data);
}
