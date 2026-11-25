//! Start the control API and the platform roles that run beside it.

use sqlbuild_orchestration::api::main::serve_api::serve_api;
use sqlbuild_orchestration::api::models::{ApiAuthentication, ServerFacts};
use sqlbuild_orchestration::health::models::ServerHealth;
use sqlbuild_orchestration::platform::constants::PLATFORM_WEBHOOK_VARIABLE;
use sqlbuild_orchestration::platform::main::installation::run_maintenance::run_maintenance;
use sqlbuild_orchestration::platform::models::RetentionPolicies;
use sqlbuild_orchestration::run_output::models::OutputStore;
use sqlbuild_orchestration::runs::types::RunHandoff;
use sqlbuild_orchestration::store::errors::ControlError;
use sqlbuild_orchestration::store::models::ControlStore;
use sqlbuild_orchestration::webhooks::constants::SECURITY_AUDIT_WEBHOOK_VARIABLE;
use sqlbuild_orchestration::webhooks::main::connect_webhook::connect_webhook;
use sqlbuild_orchestration::webhooks::main::run_webhook::run_webhook;
use sqlbuild_orchestration::webhooks::models::{Webhook, WebhookSource};
use tokio::sync::watch;
use tokio::task::JoinSet;
use uuid::Uuid;

use crate::argv::models::Stream;
use crate::native_commands::_helpers::command::terminal::write_output;
use crate::native_commands::models::ApiStandby;

/// Resolve once `shutdown` turns true or its sender is gone.
async fn stopped(mut shutdown: watch::Receiver<bool>) {
    while !*shutdown.borrow_and_update() {
        if shutdown.changed().await.is_err() {
            break;
        }
    }
}

/// Start the control API and the platform roles beside it, with the retention `facts` and `run_output` set.
pub(crate) async fn start_api(
    mut tasks: JoinSet<Result<(), ControlError>>,
    (standby, store, shutdown): (ApiStandby, &ControlStore, &watch::Receiver<bool>),
    (authentication, handoff, facts): (ApiAuthentication, RunHandoff, &ServerFacts),
    run_output: Option<(OutputStore, u16)>,
) -> Result<JoinSet<Result<(), ControlError>>, ControlError> {
    let ApiStandby { listener, end } = standby;
    let address = listener.local_addr()?;
    tasks.spawn(serve_api(
        store.clone(),
        listener,
        (authentication, handoff, facts.clone()),
        (
            move || {
                drop(write_output(&[(
                    Stream::Stderr,
                    &format!("Control API ready: {address}\n"),
                )]));
            },
            stopped(shutdown.clone()),
        ),
    ));
    end.send(()).unwrap_or_default();
    start_platform_roles(
        &mut tasks,
        (store, shutdown, Some(&facts.health)),
        RetentionPolicies {
            audit: facts.audit,
            statistics: facts.statistics,
            snapshots: facts.snapshots,
            run_logs: facts.run_logs,
            state_history: facts.state_history,
            run_output,
        },
    )?;
    Ok(tasks)
}

/// Health production and delivery remain native and independent of scheduler leadership.
pub(crate) fn start_platform_roles(
    tasks: &mut JoinSet<Result<(), ControlError>>,
    (store, shutdown, health): (&ControlStore, &watch::Receiver<bool>, Option<&ServerHealth>),
    retention: RetentionPolicies,
) -> Result<(), ControlError> {
    let mut sinks: Vec<(Webhook, &str)> = Vec::new();
    for (variable, source, label) in [
        (
            PLATFORM_WEBHOOK_VARIABLE,
            WebhookSource::Platform,
            "Platform",
        ),
        (
            SECURITY_AUDIT_WEBHOOK_VARIABLE,
            WebhookSource::SecurityAudit,
            "Security audit",
        ),
    ] {
        match std::env::var(variable) {
            Ok(endpoint) => sinks.push((connect_webhook(&endpoint, source)?, label)),
            Err(std::env::VarError::NotPresent) => {}
            Err(_) => {
                return Err(ControlError::Invalid(format!(
                    "{label} webhook configuration must be UTF-8"
                )));
            }
        }
    }
    drop(write_output(&[(
        Stream::Stderr,
        "Starting platform maintenance...\n",
    )]));
    let maintenance_store = store.clone();
    let maintenance_shutdown = shutdown.clone();
    let audit_sinks: Vec<Webhook> = sinks.iter().map(|(sink, _)| sink.clone()).collect();
    tasks.spawn(async move {
        let report = |retained: i64| {
            drop(write_output(&[(
                Stream::Stderr,
                &format!("Security audit retention: retained past horizon: {retained}\n"),
            )]));
        };
        run_maintenance(
            (&maintenance_store, Uuid::now_v7()),
            (retention, &audit_sinks),
            (report, maintenance_shutdown),
        )
        .await
    });
    drop(write_output(&[(
        Stream::Stderr,
        "Platform maintenance started\n",
    )]));
    for (sink, label) in sinks {
        drop(write_output(&[(
            Stream::Stderr,
            &format!("Starting {label} webhook...\n"),
        )]));
        tasks.spawn(run_webhook(
            (store.clone(), health.cloned()),
            sink,
            shutdown.clone(),
        ));
        drop(write_output(&[(
            Stream::Stderr,
            &format!("{label} webhook started\n"),
        )]));
    }
    Ok(())
}
