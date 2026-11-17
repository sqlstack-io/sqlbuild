//! The `statements` section of the `sqb test` and `sqb scenario test` JSON documents.

use serde_json::{Value, json};

use crate::progress::constants::STATEMENT_TIMING_SOURCE;
use crate::progress::models::{StatementSummary, StatementTiming};

/// `{total, by_kind, compile_time}`; `compile_time` is null when the adapter reports none.
#[must_use]
pub fn statement_summary_json(summary: &StatementSummary, timing: &StatementTiming) -> Value {
    let compile_time: Value = match timing {
        StatementTiming::Reported(timings) => json!({
            "source": STATEMENT_TIMING_SOURCE,
            "compile_ms": timings.compile_ms,
            "execution_ms": timings.execution_ms,
            "timed_statements": timings.timed_statements,
            "identified_statements": summary.query_ids.len(),
        }),
        StatementTiming::NotReported(_) => Value::Null,
        StatementTiming::Failed(reason) => json!({
            "source": STATEMENT_TIMING_SOURCE,
            "error": reason,
        }),
    };
    json!({
        "total": summary.by_kind.values().sum::<u64>(),
        "by_kind": summary.by_kind,
        "compile_time": compile_time,
    })
}
