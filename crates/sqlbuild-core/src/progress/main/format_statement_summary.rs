//! The statement summary line of `sqb test` and `sqb scenario test`.

use crate::progress::models::{StatementSummary, StatementTiming};

/// `Statements: 12 on the warehouse (SELECT 8, CREATE 2, DROP 2); compile 1.5s, execution 0.8s`; nothing without statements.
#[must_use]
pub fn format_statement_summary(
    summary: &StatementSummary,
    timing: &StatementTiming,
) -> Option<String> {
    let total: u64 = summary.by_kind.values().sum();
    if total == 0 {
        return None;
    }
    let mut kinds: Vec<(&String, &u64)> = summary.by_kind.iter().collect();
    kinds.sort_by(|(left_kind, left), (right_kind, right)| {
        right.cmp(left).then_with(|| left_kind.cmp(right_kind))
    });
    let kinds: String = kinds
        .iter()
        .map(|(kind, count)| format!("{kind} {count}"))
        .collect::<Vec<_>>()
        .join(", ");
    let time: String = match timing {
        StatementTiming::Reported(timings) => {
            let identified: u64 = u64::try_from(summary.query_ids.len()).unwrap_or(u64::MAX);
            let coverage: String = if timings.timed_statements < identified {
                format!(
                    " ({} of {identified} statements in query history)",
                    timings.timed_statements
                )
            } else {
                String::new()
            };
            format!(
                "compile {:.1}s, execution {:.1}s{coverage}",
                timings.compile_ms / 1000.0,
                timings.execution_ms / 1000.0
            )
        }
        StatementTiming::NotReported(adapter) => format!("compile time not reported by {adapter}"),
        StatementTiming::Failed(reason) => format!("compile time unavailable ({reason})"),
    };
    Some(format!("Statements: {total} on the warehouse ({kinds}); {time}"))
}
