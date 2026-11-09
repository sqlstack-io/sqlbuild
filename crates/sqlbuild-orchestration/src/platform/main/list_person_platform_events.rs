//! People see server-wide platform events, and a location target's events only for targets they list.

use std::collections::BTreeMap;

use uuid::Uuid;

use crate::authorization::main::person_holds_target::person_holds_target;
use crate::credentials::models::{Action, DefaultRole};
use crate::platform::_helpers::event_history::event_history;
use crate::platform::models::PersonPlatformEvent;
use crate::store::errors::ControlError;
use crate::store::main::{begin::begin, commit::commit};
use crate::store::models::{ControlStore, TransactionMode};

/// Newest first; `before` continues after the last storage ID of a previous page, which can come back shorter than `limit`.
pub async fn list_person_platform_events(
    store: &ControlStore,
    principal_id: Uuid,
    page: (Option<i64>, u32),
    default_role: DefaultRole,
) -> Result<Vec<PersonPlatformEvent>, ControlError> {
    let (transaction, history) =
        event_history(begin(store, TransactionMode::Read).await?, page).await?;
    commit(transaction).await?;
    let mut listed: BTreeMap<(Uuid, String), bool> = BTreeMap::new();
    let mut visible: Vec<PersonPlatformEvent> = Vec::new();
    for event in history {
        let (Some(location_id), Some(target)) = (event.location_id, event.target.clone()) else {
            visible.push(event);
            continue;
        };
        let shown = match listed.get(&(location_id, target.clone())) {
            Some(shown) => *shown,
            None => {
                let shown = person_holds_target(
                    store,
                    principal_id,
                    (Action::View, location_id, &target),
                    default_role,
                )
                .await?;
                listed.insert((location_id, target), shown);
                shown
            }
        };
        if shown {
            visible.push(event);
        }
    }
    Ok(visible)
}
