//! Lifecycle event sinks whose newest run lost events, from the runs' sink delivery reports.

use std::collections::BTreeMap;

use chrono::TimeDelta;
use uuid::Uuid;

use crate::platform::constants::{SINK_PROVIDERS, SINK_REPORT_LIMIT, SINK_REPORT_WINDOW_DAYS};
use crate::platform::models::{FailingSink, SinkReport, SinkReportRow, SinkStreak};
use crate::runs::constants::SINK_DELIVERIES_EVENT;
use crate::runs::models::{RunChange, SinkDelivery};
use crate::store::errors::ControlError;
use crate::store::main::decode_row::decode_row;
use crate::store::main::query::query;
use crate::store::models::{ControlTransaction, Parameter};

/// The sink delivery reports of the last week's runs at live locations, newest first.
pub(crate) async fn sink_reports(
    transaction: ControlTransaction,
) -> Result<(ControlTransaction, Vec<SinkReport>), ControlError> {
    let since = transaction.now - TimeDelta::days(SINK_REPORT_WINDOW_DAYS);
    let (transaction, rows) = query(
        transaction,
        "SELECT e.occurred_at, e.payload, r.location_id, l.name AS location_name, r.target \
         FROM recall_events e JOIN sqb_runs r ON CAST(r.run_id AS TEXT)=e.run_id \
         JOIN sqb_locations l ON l.location_id=r.location_id \
         WHERE e.event_type=$1 AND e.occurred_at>=$2 AND l.archived_at IS NULL \
         ORDER BY e.storage_id DESC LIMIT $3",
        vec![
            Parameter::Text(SINK_DELIVERIES_EVENT.to_owned()),
            Parameter::Time(since),
            Parameter::Integer(SINK_REPORT_LIMIT),
        ],
    )
    .await?;
    let rows: Vec<SinkReportRow> = rows.into_iter().map(decode_row).collect::<Result<_, _>>()?;
    Ok((
        transaction,
        rows.into_iter().filter_map(sink_report).collect(),
    ))
}

/// A row as a report; a stored change that isn't a sink delivery report is none.
fn sink_report(row: SinkReportRow) -> Option<SinkReport> {
    let RunChange::SinkDeliveries {
        sinks,
        teardown_failures,
    } = row.payload.change.change
    else {
        return None;
    };
    Some(SinkReport {
        location_id: row.location_id,
        location_name: row.location_name,
        target: row.target,
        run_id: row.payload.change.run_id,
        at: row.occurred_at,
        sinks,
        teardown_failures,
    })
}

/// Each sink whose newest report lost events, since the first lossy report after its newest lossless one.
pub(crate) fn failing_sinks(reports: &[SinkReport]) -> Vec<FailingSink> {
    let mut streaks: BTreeMap<(Uuid, String, String), SinkStreak> = BTreeMap::new();
    for report in reports {
        for (name, reason) in losses(report) {
            let key = (report.location_id, report.target.clone(), name.clone());
            let streak = streaks.remove(&key).map_or_else(
                || SinkStreak::first(report, (name, reason.clone())),
                |streak| streak.older(report.at, reason.is_some()),
            );
            streaks.insert(key, streak);
        }
    }
    streaks
        .into_values()
        .filter_map(SinkStreak::failing)
        .collect()
}

/// Each sink in `report` with what it lost, then the sink providers with their teardown failures.
fn losses(report: &SinkReport) -> Vec<(String, Option<String>)> {
    let providers: Option<String> = (!report.teardown_failures.is_empty()).then(|| {
        format!(
            "closing failed, so events they held may be lost: {}",
            report.teardown_failures.join(", ")
        )
    });
    report
        .sinks
        .iter()
        .map(|sink| (sink.name.clone(), loss_reason(sink)))
        .chain([(SINK_PROVIDERS.to_owned(), providers)])
        .collect()
}

/// `8 of 8 events not delivered (8 failed)`; `None` when the sink delivered every event.
fn loss_reason(sink: &SinkDelivery) -> Option<String> {
    let parts: Vec<String> = [(sink.dropped, "dropped"), (sink.failed, "failed")]
        .into_iter()
        .filter(|(count, _)| *count > 0)
        .map(|(count, label)| format!("{count} {label}"))
        .collect();
    (!parts.is_empty()).then(|| {
        format!(
            "{} of {} events not delivered ({})",
            sink.dropped + sink.failed,
            sink.accepted,
            parts.join(", ")
        )
    })
}

impl SinkStreak {
    /// A sink's newest report: failing when it lost events.
    fn first(report: &SinkReport, (sink, reason): (String, Option<String>)) -> Self {
        reason.map_or(Self::Clear, |reason| {
            Self::Failing(FailingSink {
                location_id: report.location_id,
                location_name: report.location_name.clone(),
                target: report.target.clone(),
                sink,
                since: report.at,
                reason,
                run_id: report.run_id,
                last_success_at: None,
            })
        })
    }

    /// The streak with an older report at `at` added: a lossy one moves its start back, a lossless one ends it.
    fn older(self, at: chrono::DateTime<chrono::Utc>, lossy: bool) -> Self {
        match self {
            Self::Failing(failing) if lossy => Self::Failing(FailingSink {
                since: at,
                ..failing
            }),
            Self::Failing(failing) => Self::Ended(FailingSink {
                last_success_at: Some(at),
                ..failing
            }),
            other => other,
        }
    }

    /// The sink's problem, if its newest report lost events.
    fn failing(self) -> Option<FailingSink> {
        match self {
            Self::Failing(failing) | Self::Ended(failing) => Some(failing),
            Self::Clear => None,
        }
    }
}
