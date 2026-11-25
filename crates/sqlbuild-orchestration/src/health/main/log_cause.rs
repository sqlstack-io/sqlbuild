//! Write a control database failure to the server log by each cause's own message, never a stack trace.

use crate::health::models::ControlCause;

/// One log line for `cause`, naming the request that met it when one did.
pub fn log_cause(request_id: Option<uuid::Uuid>, cause: &ControlCause) {
    let request: String = request_id.map_or_else(String::new, |id| format!(" (request {id})"));
    eprintln!("{}{request}: {}", cause.message, cause.chain.join(": "));
}
