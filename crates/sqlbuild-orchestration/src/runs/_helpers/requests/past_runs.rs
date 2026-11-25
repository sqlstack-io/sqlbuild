//! The past runs `run:`, `failed:` and `skipped:` name, found by full ID or unique prefix in one location and target.

use std::collections::BTreeSet;

use uuid::Uuid;

use crate::runs::constants::{
    CHECK_KIND, FAILED_RUN_NODE_STATUSES, MAX_LISTED_RUN_MATCHES, MIN_RUN_ID_PREFIX,
    NOT_RUN_STATUS, PAST_RUN_AUDIT_LIMIT, SKIPPED_RUN_NODE_STATUS,
};
use crate::runs::models::{PastRunResults, SelectionProblem};
use crate::runs::types::PastRuns;
use crate::runs::types::PreparedPublication;
use crate::store::errors::ControlError;
use crate::store::main::begin::begin;
use crate::store::main::commit::commit;
use crate::store::main::query::query;
use crate::store::models::{ControlStore, ControlTransaction, Parameter, TransactionMode};
use crate::warehouse_history::main::runs::read_run_failure_recall::read_run_failure_recall;

/// Each run `ids` names in `(location_id, target)`, or why one can't be read: too short, unknown or ambiguous.
pub(crate) async fn read_past_runs(
    mut transaction: ControlTransaction,
    (location_id, target): (Uuid, &str),
    ids: &BTreeSet<String>,
) -> Result<(ControlTransaction, Result<PastRuns, String>), ControlError> {
    let mut past = PastRuns::new();
    for id in ids {
        let found;
        (transaction, found) = matching_run(transaction, (location_id, target), id).await?;
        let run_id = match found {
            Ok(run_id) => run_id,
            Err(refusal) => return Ok((transaction, Err(refusal))),
        };
        let results;
        (transaction, results) = run_results(transaction, run_id).await?;
        past.insert(id.clone(), results);
    }
    Ok((transaction, Ok(past)))
}

/// The past runs `select` and `exclude` name, read in their own transaction; a run that can't be read is a selection problem.
pub(crate) async fn read_selected_past_runs(
    store: &ControlStore,
    (location_id, target): (Uuid, &str),
    (prepared, select, exclude): (&dyn PreparedPublication, &str, &str),
) -> Result<Result<PastRuns, Box<SelectionProblem>>, ControlError> {
    let ids = prepared.run_selector_ids(&[select.to_owned(), exclude.to_owned()]);
    if ids.is_empty() {
        return Ok(Ok(PastRuns::new()));
    }
    let transaction = begin(store, TransactionMode::Read).await?;
    let (transaction, past) = read_past_runs(transaction, (location_id, target), &ids).await?;
    commit(transaction).await?;
    Ok(past.map_err(|message| {
        Box::new(SelectionProblem {
            code: None,
            message,
            help: None,
            location: None,
            suggestions: Vec::new(),
            refused: None,
        })
    }))
}

/// The one run whose ID is or starts with `id`, or why there isn't exactly one.
async fn matching_run(
    transaction: ControlTransaction,
    (location_id, target): (Uuid, &str),
    id: &str,
) -> Result<(ControlTransaction, Result<Uuid, String>), ControlError> {
    let prefix = id.to_ascii_lowercase();
    if prefix.len() < MIN_RUN_ID_PREFIX {
        return Ok((
            transaction,
            Err(format!(
                "run ID '{id}' is too short: give the full ID or at least {MIN_RUN_ID_PREFIX} characters of it"
            )),
        ));
    }
    if !prefix
        .bytes()
        .all(|byte| byte.is_ascii_hexdigit() || byte == b'-')
    {
        return Ok((transaction, Err(format!("'{id}' is not a run ID"))));
    }
    let (transaction, rows) = query(
        transaction,
        &format!(
            "SELECT run_id FROM sqb_runs WHERE location_id=$1 AND target=$2 \
             AND CAST(run_id AS TEXT) LIKE $3 ORDER BY run_id LIMIT {}",
            MAX_LISTED_RUN_MATCHES + 1
        ),
        vec![
            Parameter::Id(location_id),
            Parameter::Text(target.to_owned()),
            Parameter::Text(format!("{prefix}%")),
        ],
    )
    .await?;
    let matches: Vec<String> = rows
        .iter()
        .filter_map(|row| row["run_id"].as_str().map(str::to_owned))
        .collect();
    let refusal = match matches.as_slice() {
        [only] => {
            let run_id = Uuid::parse_str(only)
                .map_err(|error| ControlError::Schema(format!("stored run ID: {error}")))?;
            return Ok((transaction, Ok(run_id)));
        }
        [] => format!("no run '{id}' in this location and target"),
        many => {
            let shown: Vec<&str> = many
                .iter()
                .take(MAX_LISTED_RUN_MATCHES)
                .map(String::as_str)
                .collect();
            let more = if many.len() > MAX_LISTED_RUN_MATCHES {
                ", and more"
            } else {
                ""
            };
            format!(
                "run ID '{id}' is ambiguous: it starts {}{more}; give more of the ID",
                shown.join(", ")
            )
        }
    };
    Ok((transaction, Err(refusal)))
}

/// What one run selected, didn't finish and skipped, with its failed audits as `audit:<name>`.
async fn run_results(
    transaction: ControlTransaction,
    run_id: Uuid,
) -> Result<(ControlTransaction, PastRunResults), ControlError> {
    let (transaction, rows) = query(
        transaction,
        "SELECT node_key, status, kind FROM sqb_run_nodes WHERE run_id=$1",
        vec![Parameter::Id(run_id)],
    )
    .await?;
    let mut results = PastRunResults {
        run_id,
        ..PastRunResults::default()
    };
    for row in rows {
        let (Some(key), Some(status)) = (row["node_key"].as_str(), row["status"].as_str()) else {
            continue;
        };
        if row["kind"].as_str() == Some(CHECK_KIND) && status == NOT_RUN_STATUS {
            continue;
        }
        results.selected.insert(key.to_owned());
        if FAILED_RUN_NODE_STATUSES.contains(&status) {
            results.failed.insert(key.to_owned());
        } else if status == SKIPPED_RUN_NODE_STATUS {
            results.skipped.insert(key.to_owned());
        }
    }
    let (transaction, recall) =
        read_run_failure_recall(transaction, (run_id, PAST_RUN_AUDIT_LIMIT)).await?;
    results.failed.extend(
        recall
            .audits
            .into_iter()
            .map(|audit| format!("audit:{}", audit.name)),
    );
    Ok((transaction, results))
}
