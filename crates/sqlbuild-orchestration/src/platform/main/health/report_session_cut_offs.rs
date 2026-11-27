//! Report the sessions the control database cut off: the control database reads as failing, and each becomes a platform event alerts can act on.

use serde_json::Value;
use uuid::Uuid;

use crate::health::main::log_cause::log_cause;
use crate::health::main::return_session_cut_offs::return_session_cut_offs;
use crate::health::main::take_session_cut_offs::take_session_cut_offs;
use crate::health::models::{ControlCause, ControlCauseCase, ServerHealth, SessionCutOff};
use crate::platform::_helpers::health::observations::observe;
use crate::platform::models::PlatformEvent;
use crate::store::errors::ControlError;
use crate::store::main::{begin::begin, commit::commit, query::query};
use crate::store::models::{ControlStore, TransactionMode};

/// Log and record every session cut off since the last report; `true` when there was one, so the probe leaves the control database failing until the next round.
pub async fn report_session_cut_offs(
    store: &ControlStore,
    health: &ServerHealth,
) -> Result<bool, ControlError> {
    let cut_offs = take_session_cut_offs(store);
    if cut_offs.is_empty() {
        return Ok(false);
    }
    for cut_off in &cut_offs {
        let cause = health.locate(ControlCause {
            case: ControlCauseCase::SessionCutOff,
            message: cut_off.error.clone(),
            chain: vec![cut_off.error.clone()],
            transient: true,
            waited_for_pool: false,
        });
        health.control_failed(&cause, (cut_off.at, None));
        log_cause(None, &cause);
    }
    if let Err(error) = record(store, &cut_offs).await {
        return_session_cut_offs(store, cut_offs);
        return Err(error);
    }
    Ok(true)
}

/// One `control_session_cut_off` platform event per cut-off session, as alerts read them.
async fn record(store: &ControlStore, cut_offs: &[SessionCutOff]) -> Result<(), ControlError> {
    let transaction = begin(store, TransactionMode::Write).await?;
    let (mut transaction, rows) = query(
        transaction,
        "SELECT installation_id FROM sqb_installation WHERE singleton=1",
        Vec::new(),
    )
    .await?;
    let installation = rows
        .first()
        .and_then(|row| row.get("installation_id"))
        .and_then(Value::as_str)
        .ok_or_else(|| ControlError::Invalid("control store has no installation".into()))?;
    let installation_id = Uuid::parse_str(installation)
        .map_err(|error| ControlError::Invalid(format!("installation ID: {error}")))?;
    for cut_off in cut_offs {
        transaction = observe(
            transaction,
            installation_id,
            (
                PlatformEvent::ControlSessionCutOff {
                    idle: cut_off.idle,
                    cut_at: cut_off.at,
                    error: cut_off.error.clone(),
                },
                Some(cut_off.at),
            ),
        )
        .await?;
    }
    commit(transaction).await
}
