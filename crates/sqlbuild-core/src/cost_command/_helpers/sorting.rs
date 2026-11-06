//! Stable cost ordering with cached exact decimals and UTC instants.

use crate::cost_command::constants::{HISTORY_METRIC_SORT_FIELDS, MISSING_METRIC_STATUSES};
use crate::cost_command::errors::CostCommandError;
use crate::cost_command::models::CostSortMetric;
use crate::cost_report::main::cost_decimal_key::cost_decimal_key;
use crate::cost_report::main::cost_timestamp::cost_timestamp;
use crate::cost_report::models::{CostResource, CostRun};

fn invalid() -> CostCommandError {
    CostCommandError::new("C000", "Invalid persisted cost metric or timestamp")
}

pub(crate) fn timestamp(value: &str) -> Result<i128, CostCommandError> {
    cost_timestamp(value).ok_or_else(invalid)
}

fn decimal(value: &str) -> Result<CostSortMetric, CostCommandError> {
    cost_decimal_key(value)
        .map(CostSortMetric::Decimal)
        .ok_or_else(invalid)
}

fn history_metric(
    run: &CostRun,
    field: &str,
    completed: i128,
) -> Result<CostSortMetric, CostCommandError> {
    Ok(match field {
        "cost" => return decimal(&run.cost.estimated_usd),
        "credits" => return decimal(&run.cost.estimated_compute_credits),
        "duration" => CostSortMetric::Integer(completed - timestamp(&run.started_at)?),
        "models" => CostSortMetric::Integer(run.cost.resources.len() as i128),
        "status" => CostSortMetric::Text(run.cost.status.clone()),
        _ => CostSortMetric::Integer(completed),
    })
}

pub(crate) fn history(
    runs: Vec<CostRun>,
    field: &str,
    descending: bool,
) -> Result<Vec<CostRun>, CostCommandError> {
    let mut keyed = Vec::with_capacity(runs.len());
    for run in runs {
        let completed = timestamp(&run.completed_at)?;
        let missing = HISTORY_METRIC_SORT_FIELDS.contains(&field)
            && MISSING_METRIC_STATUSES.contains(&run.cost.status.as_str());
        let metric = if missing {
            None
        } else {
            Some(history_metric(&run, field, completed)?)
        };
        keyed.push((run, metric, completed));
    }
    keyed.sort_by(
        |(left, left_key, left_time), (right, right_key, right_time)| {
            let metric = left_key.cmp(right_key);
            left_key.is_none().cmp(&right_key.is_none()).then_with(|| {
                (if descending { metric.reverse() } else { metric })
                    .then_with(|| right_time.cmp(left_time))
                    .then_with(|| left.run_id.cmp(&right.run_id))
            })
        },
    );
    Ok(keyed.into_iter().map(|(run, _, _)| run).collect())
}

fn resource_metric(
    resource: &CostResource,
    field: &str,
) -> Result<CostSortMetric, CostCommandError> {
    Ok(match field {
        "credits" => return decimal(&resource.estimated_compute_credits),
        "busy" => return decimal(&resource.attributed_seconds),
        "scanned" => CostSortMetric::Integer(i128::from(resource.bytes_scanned)),
        "model" => CostSortMetric::Text(resource.resource_name.clone()),
        "warehouse" => CostSortMetric::Text(resource.warehouse_name.clone()),
        _ => return decimal(&resource.estimated_usd),
    })
}

pub(crate) fn resources(
    resources: Vec<CostResource>,
    field: &str,
    descending: bool,
) -> Result<Vec<CostResource>, CostCommandError> {
    let mut keyed = Vec::with_capacity(resources.len());
    for resource in resources {
        let metric = resource_metric(&resource, field)?;
        keyed.push((resource, metric));
    }
    keyed.sort_by(|(left, left_key), (right, right_key)| {
        let metric = left_key
            .cmp(right_key)
            .then_with(|| left.resource_name.cmp(&right.resource_name))
            .then_with(|| left.warehouse_name.cmp(&right.warehouse_name));
        if descending { metric.reverse() } else { metric }
    });
    Ok(keyed.into_iter().map(|(resource, _)| resource).collect())
}
