//! Scoped invalidations, not a replayable event log or a second source of application data.

use std::time::Duration;

use axum::Extension;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, HeaderValue};
use axum::response::Sse;
use axum::response::sse::Event;
use futures_util::{Stream, stream};
use sqlbuild_recall::records::main::sha256_hex::sha256_hex;
use tokio::sync::broadcast;
use uuid::Uuid;

use crate::api::_helpers::authentication::bearer::{bearer, require_location};
use crate::api::errors::ApiError;
use crate::api::models::{ChangeFeed, ChangeNotice, ChangeStream};
use crate::credentials::main::authenticate_token::authenticate_token;
use crate::credentials::models::{CredentialScope, TokenIdentity};
use crate::store::errors::ControlError;
use crate::store::main::listen_changes::listen_changes;
use crate::store::models::{ChangeMessage, ChangeTopic, ControlStore};

#[utoipa::path(get, path = "/api/locations/{location_id}/targets/{target}/changes",
    params(("location_id" = Uuid, Path), ("target" = String, Path)),
    responses((status = 200, body = ChangeNotice, content_type = "text/event-stream",
        description = "changed events: all, runs, run:<id>, ticks, snapshot:<location>, node_status:<target>. Refetch REST on all/reconnect; no durable cursor."),
        (status = 401, description = "Invalid credential"),
        (status = 403, description = "Location not authorized")), security(("bearer" = [])))]
pub(crate) async fn subscribe(
    State(store): State<ControlStore>,
    (Extension(identity), Extension(feed)): (Extension<TokenIdentity>, Extension<ChangeFeed>),
    Path((location_id, target)): Path<(Uuid, String)>,
    headers: HeaderMap,
) -> Result<Sse<impl Stream<Item = Result<Event, ControlError>>>, ApiError> {
    require_location(&identity, location_id)?;
    let authorization = if identity.token_id.is_nil() {
        None
    } else {
        let mut authorization = HeaderValue::from_str(bearer(&headers)?)
            .map_err(|error| ControlError::Invalid(error.to_string()))?;
        authorization.set_sensitive(true);
        Some(authorization)
    };
    let state = ChangeStream {
        store,
        authorization,
        location_id,
        target_hash: sha256_hex(target.as_bytes()),
        target,
        receiver: feed.sender.subscribe(),
        stopped: feed.stopped,
        keepalive: tokio::time::interval(Duration::from_secs(15)),
        initial: true,
    };
    Ok(Sse::new(stream::try_unfold(state, next_event)))
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
                Some(match topic {
                    ChangeTopic::Runs => "runs".into(),
                    ChangeTopic::Run(id) => format!("run:{id}"),
                    ChangeTopic::Ticks => "ticks".into(),
                    ChangeTopic::Snapshot => format!("snapshot:{}", state.location_id),
                    ChangeTopic::NodeStatus => format!("node_status:{}", state.target),
                })
            }
            Some(ChangeMessage::Resync) => Some("all".into()),
            Some(ChangeMessage::CredentialsChanged) | None => None,
        };
        if let Some(authorization) = &state.authorization {
            let token = authorization
                .to_str()
                .map_err(|error| ControlError::Invalid(error.to_string()))?;
            let Some(identity) = authenticate_token(&state.store, token).await? else {
                return Ok(None);
            };
            if identity.scope
                != (CredentialScope::Location {
                    location_id: state.location_id,
                })
            {
                return Ok(None);
            }
        }
        if let Some(topic) = topic {
            let event = Event::default()
                .event("changed")
                .json_data(ChangeNotice { topic })
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
                Err(ControlError::Database(error)) if transient_listener_error(&error) => continue,
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
            Err(error) if transient_listener_error(&error) => {
                listener = None;
                continue;
            }
            Err(error) => return Err(error.into()),
        };
        let _ = sender.send(message);
    }
}

fn transient_listener_error(error: &sqlx_core::Error) -> bool {
    match error {
        sqlx_core::Error::Io(_)
        | sqlx_core::Error::PoolTimedOut
        | sqlx_core::Error::WorkerCrashed => true,
        sqlx_core::Error::Database(error) => error.code().is_some_and(|code| {
            code.starts_with("08")
                || code.starts_with("53")
                || matches!(code.as_ref(), "57P01" | "57P02" | "57P03")
        }),
        _ => false,
    }
}
