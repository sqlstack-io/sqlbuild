//! Read the past runs a command's selectors name, from the server it names or the local Recall history.

use std::collections::BTreeSet;

use sqlbuild_recall::store::main::open_sqlite_store::open_sqlite_store;
use sqlbuild_recall::store::main::read_run_results::read_run_results;
use sqlbuild_recall::store::main::resolve_run::resolve_run;
use sqlbuild_recall::store::models::OpenMode;

use crate::person_login::main::read_past_runs::read_past_runs;
use crate::run_selectors::models::{PastRunKeys, PastRunSource};
use crate::run_selectors::types::PastRunKeysById;

/// What each run `ids` names selected, didn't finish and skipped, or why one can't be read.
pub fn read_past_run_keys(
    source: &PastRunSource,
    ids: &BTreeSet<String>,
) -> Result<PastRunKeysById, String> {
    match source {
        PastRunSource::Server {
            origin,
            ca,
            location_id,
            target,
        } => {
            let ids: Vec<String> = ids.iter().cloned().collect();
            let read = read_past_runs((origin, ca.as_deref()), (*location_id, target), &ids)
                .map_err(|error| error.to_string())?;
            if let Some(error) = read.error {
                return Err(error);
            }
            Ok(read
                .runs
                .into_iter()
                .map(|(id, run)| {
                    (
                        id,
                        PastRunKeys {
                            selected: run.selected,
                            failed: run.failed,
                            skipped: run.skipped,
                        },
                    )
                })
                .collect())
        }
        PastRunSource::Recall { store } => {
            let store = open_sqlite_store(store, OpenMode::ReadOnly).map_err(|error| {
                format!(
                    "can't read the local Recall history for run selectors: {}",
                    error.message
                )
            })?;
            let mut runs = PastRunKeysById::new();
            for id in ids {
                let run_id = resolve_run(&store, id).map_err(|error| error.message)?;
                let results = read_run_results(&store, &run_id).map_err(|error| error.message)?;
                runs.insert(
                    id.clone(),
                    PastRunKeys {
                        selected: results.selected,
                        failed: results.failed,
                        skipped: results.skipped,
                    },
                );
            }
            Ok(runs)
        }
        PastRunSource::Unavailable(reason) => Err(reason.clone()),
    }
}
