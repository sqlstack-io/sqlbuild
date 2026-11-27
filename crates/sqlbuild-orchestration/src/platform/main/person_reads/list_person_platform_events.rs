//! People see server-wide platform events, and a location target's events only for targets they list.

use std::collections::BTreeMap;

use uuid::Uuid;

use crate::credentials::models::DefaultRole;
use crate::platform::_helpers::health::event_history::{Visibility, event_history};
use crate::platform::models::{
    PersonPlatformEvent, PersonPlatformEventPage, UnreadablePlatformEvent,
};
use crate::store::errors::ControlError;
use crate::store::main::{begin::begin, commit::commit};
use crate::store::models::{ControlStore, TransactionMode};

/// Newest first: one scan of `limit` stored events, keeping those the person may see, so a page can be empty yet continue; events that can't be read are listed apart.
pub async fn list_person_platform_events(
    store: &ControlStore,
    principal_id: Uuid,
    page: (Option<i64>, u32),
    default_role: DefaultRole,
) -> Result<PersonPlatformEventPage, ControlError> {
    let (transaction, scan) =
        event_history(begin(store, TransactionMode::Read).await?, page).await?;
    commit(transaction).await?;
    let mut visibility = Visibility {
        store,
        person: (principal_id, default_role),
        listed: BTreeMap::new(),
    };
    let mut events: Vec<PersonPlatformEvent> = Vec::new();
    for event in scan.events {
        let scope = (event.location_id, event.target.clone());
        if visibility.shown(scope).await? {
            events.push(event);
        }
    }
    let mut unreadable: Vec<UnreadablePlatformEvent> = Vec::new();
    for item in scan.unreadable {
        let scope = (item.location_id, item.target.clone());
        if visibility.shown(scope).await? {
            unreadable.push(item);
        }
    }
    Ok(PersonPlatformEventPage {
        events,
        before: scan.before,
        unreadable,
    })
}
