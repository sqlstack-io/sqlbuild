//! Project one lifecycle fact onto persistent progress lines.

use crate::progress::_helpers::operations::{
    consume_operation_start, consume_operation_terminal, retry_scheduled_line,
};
use crate::progress::_helpers::resources::{consume_resource_start, consume_resource_terminal};
use crate::progress::_helpers::statements::{
    consume_statement_start, consume_statement_submitted, consume_statement_terminal,
    count_statement, statement_heartbeat_line,
};
use crate::progress::constants::{
    OPERATION_START, RESOURCE_START, RETRY_SCHEDULED, STATEMENT_COMPLETED, STATEMENT_FAILED,
    STATEMENT_HEARTBEAT, STATEMENT_START, STATEMENT_SUBMITTED,
};
use crate::progress::models::{ProgressEvent, ProgressProjection};
use crate::terminal_events::constants::{OPERATION_TERMINALS, RESOURCE_TERMINALS};

/// Consume a fact once; also returns the lines to write, without trailing newlines.
#[must_use]
pub fn consume_progress_event(
    mut projection: ProgressProjection,
    event: &ProgressEvent,
) -> (ProgressProjection, Vec<String>) {
    if !projection.seen_event_ids.insert(event.event_id.clone()) {
        return (projection, Vec::new());
    }
    let event_type: &str = event.event_type.as_str();
    if event_type == RESOURCE_START {
        return consume_resource_start(projection, event);
    }
    if RESOURCE_TERMINALS.contains(&event_type) {
        return consume_resource_terminal(projection, event);
    }
    let (projection, line) = if event_type == OPERATION_START {
        consume_operation_start(projection, event)
    } else if OPERATION_TERMINALS.contains(&event_type) {
        consume_operation_terminal(projection, event)
    } else if event_type == RETRY_SCHEDULED {
        let line: Option<String> = retry_scheduled_line(event);
        (projection, line)
    } else if event_type == STATEMENT_START {
        consume_statement_start(projection, event)
    } else if event_type == STATEMENT_SUBMITTED {
        consume_statement_submitted(projection, event)
    } else if event_type == STATEMENT_HEARTBEAT {
        let line: Option<String> = statement_heartbeat_line(&projection, event);
        (projection, line)
    } else if event_type == STATEMENT_COMPLETED || event_type == STATEMENT_FAILED {
        consume_statement_terminal(count_statement(projection, event), event)
    } else {
        (projection, None)
    };
    (projection, line.into_iter().collect())
}
