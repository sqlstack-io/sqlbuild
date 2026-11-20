//! Stop the development roles together within a grace period, one line per service, and say how the session ended.

use std::collections::HashMap;
use std::future::Future;

use sqlbuild_orchestration::store::errors::ControlError;
use tokio::sync::mpsc::UnboundedSender;
use tokio::sync::watch;
use tokio::task::{Id, JoinError, JoinSet};
use tokio::time::Instant;

use crate::dev_screen::models::ScreenMessage;
use crate::dev_session::constants::WAREHOUSE_FAILED;
use crate::dev_stop::constants::{ABORT_WAIT, STOP_GRACE};
use crate::dev_stop::main::stop_switch::stop_switch;
use crate::dev_stop::models::DevEnd;

/// A role's end as `JoinSet::join_next_with_id` reports it; `None` when no role was left.
type RoleEnd = Option<Result<(Id, Result<(), ControlError>), JoinError>>;

/// The development roles and the name each stop line gives them.
#[derive(Default)]
pub(crate) struct DevRoles {
    pub(crate) tasks: JoinSet<Result<(), ControlError>>,
    labels: HashMap<Id, &'static str>,
}

impl DevRoles {
    /// Run `role`; a `label` such as `UI server` gives it a `Stopped UI server` line.
    pub(crate) fn spawn(
        &mut self,
        label: Option<&'static str>,
        role: impl Future<Output = Result<(), ControlError>> + Send + 'static,
    ) {
        let id: Id = self.tasks.spawn(role).id();
        if let Some(label) = label {
            self.labels.insert(id, label);
        }
    }
}

/// Wait for a stop request or a role to end, then stop every role: each gets the grace period, then its processes are killed and the rest aborted.
pub(crate) async fn supervise_development_roles(
    mut roles: DevRoles,
    stop: watch::Sender<bool>,
    messages: &UnboundedSender<ScreenMessage>,
) -> DevEnd {
    let end: DevEnd = tokio::select! {
        joined = roles.tasks.join_next_with_id() => role_ended(joined, &roles.labels),
        reason = stop_switch().requested() => DevEnd::Stopped(reason),
    };
    drop(messages.send(ScreenMessage::Stopping));
    stop.send_replace(true);
    stop_switch().terminate_groups();
    let began: Instant = Instant::now();
    loop {
        match tokio::time::timeout_at(began + STOP_GRACE, roles.tasks.join_next_with_id()).await {
            Ok(Some(joined)) => {
                if let Ok((id, _)) = &joined
                    && let Some(label) = roles.labels.get(id)
                {
                    drop(messages.send(ScreenMessage::Stopped {
                        label: format!("Stopped {label}"),
                        elapsed: began.elapsed(),
                    }));
                }
            }
            Ok(None) => break,
            Err(_) => {
                stop_switch().kill_all();
                roles.tasks.abort_all();
                while let Ok(Some(_)) =
                    tokio::time::timeout(ABORT_WAIT, roles.tasks.join_next()).await
                {
                }
                break;
            }
        }
    }
    end
}

/// Why the session ends when a role ended by itself: before ready it did not start, after it failed.
fn role_ended(joined: RoleEnd, labels: &HashMap<Id, &'static str>) -> DevEnd {
    let reason: String = match joined {
        None => "every service stopped".to_owned(),
        Some(Ok((id, Ok(())))) => format!(
            "the {} stopped unexpectedly",
            labels.get(&id).copied().unwrap_or("platform maintenance")
        ),
        Some(Ok((_, Err(ControlError::Invalid(message))))) => message,
        Some(Ok((_, Err(error)))) => error.to_string(),
        Some(Err(error)) => format!("a service crashed: {error}"),
    };
    if stop_switch().is_ready() {
        return DevEnd::Failed { reason };
    }
    let explained: bool = reason == WAREHOUSE_FAILED;
    let next: Vec<(String, String)> = if explained {
        vec![(
            "sqb debug".to_owned(),
            "test the connection on its own".to_owned(),
        )]
    } else {
        Vec::new()
    };
    DevEnd::NotStarted {
        reason,
        explained,
        next,
    }
}
