//! Scoped invalidations, not a replayable event log or a second source of application data.

use std::time::Duration;

use axum::Extension;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, HeaderName, HeaderValue, header};
use axum::response::sse::Event;
use axum::response::{IntoResponse, Response, Sse};
use futures_util::stream;
use sqlbuild_recall::records::main::sha256_hex::sha256_hex;
use tokio::sync::broadcast;
use uuid::Uuid;

use crate::api::_helpers::authentication::bearer::{bearer, require_location};
use crate::api::_helpers::authentication::password_transport::session_credentials;
use crate::api::constants::{STREAM_BUFFERING_HEADER, STREAM_CACHE_CONTROL};
use crate::api::errors::ApiError;
use crate::api::models::{ChangeFeed, ChangeNotice, ChangeStream, ChangeSubscriber};
use crate::authorization::main::decisions::authorize_listing::authorize_listing;
use crate::authorization::main::decisions::person_holds_target::person_holds_target;
use crate::credentials::main::sessions::authenticate_session::authenticate_session;
use crate::credentials::main::tokens::authenticate_token::authenticate_token;
use crate::credentials::models::{
    Action, AuditContext, AuthenticatedPerson, CredentialScope, DefaultRole, SessionAccess,
    TokenIdentity,
};
use crate::node_states::main::watch_node_states::watch_node_states;
use crate::node_states::models::NodeStateCache;
use crate::store::errors::ControlError;
use crate::store::main::listen_changes::listen_changes;
use crate::store::main::{begin::begin, commit::commit};
use crate::store::models::{ChangeMessage, ChangeTopic, ControlStore, TransactionMode};

#[utoipa::path(get, path = "/api/locations/{location_id}/targets/{target}/changes",
    params(("location_id" = Uuid, Path), ("target" = String, Path)),
    responses((status = 200, body = ChangeNotice, content_type = "text/event-stream",
        description = "changed events: all, runs, run:<id>, ticks, snapshot:<location>, node_status:<target>. Refetch REST on all/reconnect; no durable cursor. node_status carries version, previous_version and the changed states (empty when none changed), or full: refetch node states when full is set or previous_version is not the version held."),
        (status = 401, description = "Invalid credential"),
        (status = 403, description = "Location not authorized")), security(("bearer" = [])))]
pub(crate) async fn subscribe(
    State(store): State<ControlStore>,
    (Extension(identity), Extension(feed), Extension(cache)): (
        Extension<TokenIdentity>,
        Extension<ChangeFeed>,
        Extension<NodeStateCache>,
    ),
    Path((location_id, target)): Path<(Uuid, String)>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    require_location(&identity, location_id)?;
    let subscriber = if identity.token_id.is_nil() {
        ChangeSubscriber::Development
    } else {
        ChangeSubscriber::Location(sensitive(bearer(&headers)?)?)
    };
    stream_changes((store, cache), feed, subscriber, (location_id, target)).await
}

#[utoipa::path(get, path = "/api/control/locations/{location_id}/targets/{target}/changes",
    operation_id = "subscribeChanges",
    params(("location_id" = Uuid, Path), ("target" = String, Path)),
    responses((status = 200, body = ChangeNotice, content_type = "text/event-stream",
        description = "changed events: all, runs, ticks, snapshot:<location>, node_status:<target>. Refetch REST on all/reconnect; no durable cursor. node_status carries version, previous_version and the changed states (empty when none changed), or full: refetch node states when full is set or previous_version is not the version held."),
        (status = 401, description = "Session rejected or person disabled"),
        (status = 403, description = "View does not cover every node of this target")),
    security(("session" = []), ("cliSession" = [])))]
pub(crate) async fn subscribe_person(
    (State(store), Extension(cache)): (State<ControlStore>, Extension<NodeStateCache>),
    (Extension(default_role), Extension(feed)): (Extension<DefaultRole>, Extension<ChangeFeed>),
    (Extension(AuthenticatedPerson(principal_id, session_id)), Extension(context)): (
        Extension<AuthenticatedPerson>,
        Extension<AuditContext>,
    ),
    (Path((location_id, target)), headers): (Path<(Uuid, String)>, HeaderMap),
) -> Result<Response, ApiError> {
    let transaction = begin(&store, TransactionMode::Write).await?;
    let (transaction, _) = authorize_listing(
        transaction,
        (&context, principal_id),
        (location_id, &target),
        default_role,
    )
    .await?;
    commit(transaction).await?;
    let session = match session_id {
        Some(_) => Some(sensitive(session_credentials(&headers).0)?),
        None => None,
    };
    let subscriber = ChangeSubscriber::Person {
        principal_id,
        session,
        default_role,
    };
    stream_changes((store, cache), feed, subscriber, (location_id, target)).await
}

fn sensitive(value: &str) -> Result<HeaderValue, ControlError> {
    let mut value =
        HeaderValue::from_str(value).map_err(|error| ControlError::Invalid(error.to_string()))?;
    value.set_sensitive(true);
    Ok(value)
}

/// Node states load before the first event, so every state change after the first refetch is pushed.
async fn stream_changes(
    (store, cache): (ControlStore, NodeStateCache),
    feed: ChangeFeed,
    subscriber: ChangeSubscriber,
    (location_id, target): (Uuid, String),
) -> Result<Response, ApiError> {
    let receiver = feed.sender.subscribe();
    let updates = watch_node_states(&cache, (location_id, &target)).await?;
    let state = ChangeStream {
        store,
        subscriber,
        location_id,
        target_hash: sha256_hex(target.as_bytes()),
        target,
        receiver,
        updates,
        stopped: feed.stopped,
        keepalive: tokio::time::interval(Duration::from_secs(15)),
        initial: true,
    };
    let headers = [
        (header::CACHE_CONTROL, STREAM_CACHE_CONTROL),
        (HeaderName::from_static(STREAM_BUFFERING_HEADER), "no"),
    ];
    Ok((headers, Sse::new(stream::try_unfold(state, next_event))).into_response())
}

/// Whether the subscriber may still receive this stream, checked before every emission.
async fn still_authorized(state: &ChangeStream) -> Result<bool, ControlError> {
    let header = |value: &HeaderValue| {
        value
            .to_str()
            .map(str::to_owned)
            .map_err(|error| ControlError::Invalid(error.to_string()))
    };
    match &state.subscriber {
        ChangeSubscriber::Development => Ok(true),
        ChangeSubscriber::Location(token) => {
            let identity = authenticate_token(&state.store, &header(token)?).await?;
            Ok(identity.is_some_and(|identity| {
                identity.scope
                    == CredentialScope::Location {
                        location_id: state.location_id,
                    }
            }))
        }
        ChangeSubscriber::Person {
            principal_id,
            session,
            default_role,
        } => {
            if let Some(session) = session {
                let identity =
                    authenticate_session(&state.store, &header(session)?, SessionAccess::Read)
                        .await?;
                if identity.is_none_or(|identity| identity.principal_id != *principal_id) {
                    return Ok(false);
                }
            }
            person_holds_target(
                &state.store,
                *principal_id,
                (Action::View, state.location_id, &state.target),
                *default_role,
            )
            .await
        }
    }
}

/// Revalidate every emission, keeping unexpected storage failures as stream errors.
async fn next_event(
    mut state: ChangeStream,
) -> Result<Option<(Event, ChangeStream)>, ControlError> {
    loop {
        if *state.stopped.borrow() {
            return Ok(None);
        }
        let message = if std::mem::take(&mut state.initial) {
            Some(ChangeMessage::Resync)
        } else {
            tokio::select! {
                _ = state.stopped.changed() => return Ok(None),
                _ = state.keepalive.tick() => None,
                message = state.receiver.recv() => match message {
                    Ok(message) => Some(message),
                    Err(broadcast::error::RecvError::Lagged(_)) => Some(ChangeMessage::Resync),
                    Err(broadcast::error::RecvError::Closed) => return Ok(None),
                },
                update = state.updates.recv() => match update {
                    Ok(update) if update.location_id == state.location_id && update.target_hash == state.target_hash => {
                        if !still_authorized(&state).await? {
                            return Ok(None);
                        }
                        let event = Event::default().event("changed").data(&*update.data);
                        return Ok(Some((event, state)));
                    }
                    Ok(_) => continue,
                    Err(broadcast::error::RecvError::Lagged(_)) => Some(ChangeMessage::Resync),
                    Err(broadcast::error::RecvError::Closed) => return Ok(None),
                },
            }
        };
        let keepalive = message.is_none();
        let topic = match message {
            Some(ChangeMessage::Changed {
                location_id,
                target_hash,
                topic,
            }) => {
                if location_id != state.location_id || target_hash != state.target_hash {
                    continue;
                }
                let person = matches!(state.subscriber, ChangeSubscriber::Person { .. });
                if person && matches!(topic, ChangeTopic::Run(_)) {
                    continue;
                }
                match topic {
                    ChangeTopic::Runs => Some("runs".into()),
                    ChangeTopic::Run(id) => Some(format!("run:{id}")),
                    ChangeTopic::Ticks => Some("ticks".into()),
                    ChangeTopic::Snapshot => Some(format!("snapshot:{}", state.location_id)),
                    ChangeTopic::NodeStatus => continue,
                }
            }
            Some(ChangeMessage::Resync) => Some("all".into()),
            Some(ChangeMessage::CredentialsChanged) | None => None,
        };
        if !still_authorized(&state).await? {
            return Ok(None);
        }
        if let Some(topic) = topic {
            let event = Event::default()
                .event("changed")
                .json_data(ChangeNotice {
                    topic,
                    version: None,
                    previous_version: None,
                    states: None,
                    full: None,
                })
                .map_err(|error| ControlError::Invalid(error.to_string()))?;
            return Ok(Some((event, state)));
        }
        if keepalive {
            return Ok(Some((Event::default().comment("keep-alive"), state)));
        }
    }
}

pub(crate) async fn relay(
    store: ControlStore,
    mut listener: Option<sqlx_postgres::PgListener>,
    sender: broadcast::Sender<ChangeMessage>,
) -> Result<(), ControlError> {
    let ControlStore::Postgres(pool) = store else {
        return std::future::pending().await;
    };
    loop {
        if listener.is_none() {
            tokio::time::sleep(Duration::from_secs(1)).await;
            listener = match listen_changes(&pool).await {
                Ok(listener) => {
                    let _ = sender.send(ChangeMessage::Resync);
                    Some(listener)
                }
                Err(ControlError::Database(error)) if ControlError::transient_database(&error) => {
                    continue;
                }
                Err(error) => return Err(error),
            };
        }
        let Some(active) = listener.as_mut() else {
            continue;
        };
        let message = match active.try_recv().await {
            Ok(Some(notification)) => serde_json::from_str(notification.payload())
                .map_err(|error| ControlError::Invalid(error.to_string()))?,
            Ok(None) => ChangeMessage::Resync,
            Err(error) if ControlError::transient_database(&error) => {
                listener = None;
                continue;
            }
            Err(error) => return Err(error.into()),
        };
        let _ = sender.send(message);
    }
}
