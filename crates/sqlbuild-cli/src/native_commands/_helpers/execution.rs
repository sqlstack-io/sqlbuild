//! Running a Python-free command: its checks, then its output with the observed services.

use std::future::Future;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::{Duration, SystemTime};

use chrono::Utc;
use serde_json::json;
use sqlbuild_orchestration::api::constants::BODY_BYTES;
use sqlbuild_orchestration::api::errors::ClientError;
use sqlbuild_orchestration::api::main::serve_api::serve_api;
use sqlbuild_orchestration::api::models::{ApiAuthentication, DevelopmentScope, DockerImageSource};
use sqlbuild_orchestration::authorization::main::recover_administrator::recover_administrator;
use sqlbuild_orchestration::credentials::models::{AuditContext, AuditRetention};
use sqlbuild_orchestration::platform::constants::PLATFORM_WEBHOOK_VARIABLE;
use sqlbuild_orchestration::platform::main::report_remote_location_health::report_remote_location_health;
use sqlbuild_orchestration::platform::main::run_maintenance::run_maintenance;
use sqlbuild_orchestration::platform::models::{
    LocationHealth, LocationHealthReport, LocationWorkClient, LocationWorkPoll,
    WarehouseSyncFailure,
};
use sqlbuild_orchestration::scheduler::main::run_scheduler::run_scheduler;
use sqlbuild_orchestration::scheduler::models::SchedulerState;
use sqlbuild_orchestration::snapshots::main::next_remote_generation::next_remote_generation;
use sqlbuild_orchestration::snapshots::models::SnapshotRetention;
use sqlbuild_orchestration::store::errors::ControlError;
use sqlbuild_orchestration::store::main::begin_offline_recovery::begin_offline_recovery;
use sqlbuild_orchestration::store::main::migrate_store::migrate_store;
use sqlbuild_orchestration::store::main::open_store::open_store;
use sqlbuild_orchestration::store::models::ControlStore;
use sqlbuild_orchestration::warehouse_metadata::constants::MAX_HISTORY_RECORDS;
use sqlbuild_orchestration::warehouse_metadata::models::{
    MetadataBatch, MetadataScan, StatisticsDueRead, StatisticsRead, StatisticsRetention,
    WarehouseHistoryKind, WarehouseHistoryPage, WarehouseStateLocation,
};
use sqlbuild_orchestration::webhooks::constants::SECURITY_AUDIT_WEBHOOK_VARIABLE;
use sqlbuild_orchestration::webhooks::main::{
    connect_webhook::connect_webhook, run_webhook::run_webhook,
};
use sqlbuild_orchestration::webhooks::models::{Webhook, WebhookSource};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt};
use tokio::process::{Child, Command};
use tokio::sync::{mpsc, oneshot, watch};
use tokio::task::JoinSet;
use tokio::time::{Instant, MissedTickBehavior};
use uuid::Uuid;

use crate::argv::models::{Namespace, Stream, Value};
use crate::compute_logs::constants::DEFAULT_RETENTION_COUNT;
use crate::compute_logs::main::abandon_capture::abandon_capture;
use crate::compute_logs::main::finish_capture::finish_capture;
use crate::compute_logs::main::open_log_root::open_log_root;
use crate::compute_logs::main::start_capture::start_capture;
use crate::compute_logs::models::{CaptureEnd, CaptureStart, OpenCapture};
use crate::native_commands::_helpers::decision::{control_database, flag, text};
use crate::native_commands::_helpers::init::run_init;
use crate::native_commands::_helpers::package::installed_package;
use crate::native_commands::_helpers::paths::{location_database_files, project_location};
use crate::native_commands::_helpers::project_facts::has_no_runtime_extensions;
use crate::native_commands::_helpers::rules::run_rules;
use crate::native_commands::_helpers::skills::{run_skills, skill_freshness_notice};
use crate::native_commands::_helpers::terminal::{
    finish_lost_stdout, terminal_columns, write_output,
};
use crate::native_commands::constants::{
    ADMIN_COMMAND, COLUMNS_VARIABLE, FALLBACK_COLUMNS, METADATA_POLL_SECONDS,
    METADATA_RECONCILE_SECONDS, SCHEDULER_COMMAND, UNCAUGHT_EXCEPTION_STATUS, UNKNOWN_COMMAND,
};
use crate::native_commands::errors::PythonNeeded;
use crate::native_commands::main::_run_admin::run_admin;
use crate::native_commands::main::_run_control::run_control;
use crate::native_commands::main::_run_cost::run_cost;
use crate::native_commands::models::{
    CommandOutput, InstalledPackage, LocationCompileRequest, LocationCompiler,
    LocationMetadataPage, LocationMetadataSync, LocationServe, LostStdout, NativeCommand,
    NativeInvocation, ProjectLocation,
};
use crate::native_commands::types::LocationPublication;
use crate::workers::main::docker_image_sources::docker_image_sources;
use crate::workers::main::open_development_workers::open_development_workers;
use crate::workers::main::open_workers::open_workers;
use crate::workers::main::terminate_child::terminate_child;
use crate::workers::models::Workers;

/// Own the API, scheduler and outbound location as one development process tree.
pub(crate) async fn run_development_roles(
    (store, scope): (ControlStore, DevelopmentScope),
    (project, executable): (PathBuf, PathBuf),
    listener: tokio::net::TcpListener,
    scheduler: bool,
) -> Result<(), ControlError> {
    let address = listener.local_addr()?;
    let url = format!("http://{address}");
    let location_id = scope.location_id;
    let target = scope.target.clone();
    let triggers = scope.triggers;
    let workers = open_development_workers(
        (location_id, target.clone()),
        project.clone(),
        executable.clone(),
        url.clone(),
    )?;
    let (stop, shutdown) = watch::channel(false);
    let mut tasks = JoinSet::new();
    tasks.spawn(serve_api(
        store.clone(),
        listener,
        (ApiAuthentication {
            development_scope: Some(scope),
            ..ApiAuthentication::default()
        }, Vec::new(), Default::default()),
        (move || {
            drop(write_output(&[(Stream::Stderr, &format!(
                "Development API ready: {address}\nDevelopment location: {location_id}\nAuthentication disabled on loopback; triggers {}\n",
                if triggers { "follow code defaults and operator overrides" } else { "start paused" }
            ))]));
        }, development_stopped(shutdown.clone())),
    ));
    start_platform_roles(&mut tasks, (&store, &shutdown), Default::default())?;
    tasks.spawn(development_location_role(
        (&project, &executable),
        (location_id, &target),
        (&url, &store),
        shutdown.clone(),
    )?);
    if scheduler {
        drop(write_output(&[(
            Stream::Stderr,
            "Starting development scheduler...\n",
        )]));
        let scheduler_store = store.clone();
        tasks.spawn(async move {
            run_scheduler(
                &scheduler_store,
                SchedulerState::new(Uuid::now_v7()),
                shutdown,
                workers,
            )
            .await
        });
        drop(write_output(&[(
            Stream::Stderr,
            "Development scheduler started\n",
        )]));
    }
    supervise_control_roles(tasks, stop).await
}

async fn development_stopped(mut shutdown: watch::Receiver<bool>) {
    while !*shutdown.borrow_and_update() {
        if shutdown.changed().await.is_err() {
            break;
        }
    }
}

/// Hand off only the local file exclusion, never control-store connection credentials.
fn development_location_role(
    (project, executable): (&Path, &Path),
    (location_id, target): (Uuid, &str),
    (url, store): (&str, &ControlStore),
    shutdown: watch::Receiver<bool>,
) -> Result<impl Future<Output = Result<(), ControlError>> + Send + 'static, ControlError> {
    let mut command = Command::new(executable);
    command
        .args(["--project-dir"])
        .arg(project)
        .args([
            "location",
            "serve",
            "--server",
            url,
            "--location-id",
            &location_id.to_string(),
            "--target",
            target,
        ])
        .env("SQLBUILD_LOCATION_TOKEN", "local-development")
        .env_remove("SQLBUILD_CONTROL_DATABASE_URL")
        .env_remove("SQLBUILD_PLATFORM_WEBHOOK_URL")
        .env_remove(SECURITY_AUDIT_WEBHOOK_VARIABLE)
        .env_remove("SQLBUILD_RUN_TOKEN")
        .env_remove("SQLBUILD_DEVELOPMENT_CONTROL_FILE")
        .stdin(Stdio::null())
        .kill_on_drop(true);
    if let ControlStore::Sqlite(path, _) = store {
        command.env(
            "SQLBUILD_DEVELOPMENT_CONTROL_FILE",
            std::env::current_dir()?.join(path),
        );
    }
    let mut child = command.spawn()?;
    Ok(async move {
        tokio::select! {
            result = child.wait() => {
                result?;
                Err(ControlError::Invalid("development code location stopped unexpectedly".into()))
            },
            () = development_stopped(shutdown) => {
                terminate_child(&mut child)?;
                match tokio::time::timeout(Duration::from_secs(10), child.wait()).await {
                    Ok(result) => { result?; },
                    Err(_) => child.kill().await?,
                }
                Ok(())
            },
        }
    })
}

/// Request one publication without transferring interpreter ownership to a cancellable future.
pub(crate) async fn compile_generation(
    requests: &mpsc::Sender<LocationCompileRequest>,
    generation: i64,
    sources: &[sqlbuild_cache::project_snapshot::models::PathStamp],
    stopped: impl Future<Output = Result<(), ControlError>>,
) -> Result<Option<(bool, Option<Instant>, Vec<WarehouseStateLocation>)>, ControlError> {
    let (reply, publication) = oneshot::channel();
    requests
        .send(LocationCompileRequest {
            generation,
            python_sources: sources
                .iter()
                .filter(|stamp| stamp.relative_path.ends_with(".py"))
                .cloned()
                .collect(),
            reply,
        })
        .await
        .map_err(|_| ControlError::Invalid("location compiler stopped".into()))?;
    tokio::pin!(stopped);
    let result = tokio::select! {
        result = &mut stopped => { result?; return Ok(None); },
        result = publication => result.map_err(|_| ControlError::Invalid("location compiler stopped before replying".into()))?,
    };
    let record = match result {
        Ok(record) => record,
        Err(error) => {
            drop(write_output(&[(
                Stream::Stderr,
                &format!(
                    "Location compiler failed; generation {generation} may already be published: {error}\n"
                ),
            )]));
            None
        }
    };
    let published = record.is_some();
    let mut refresh_at = None;
    let mut locations: Vec<WarehouseStateLocation> = Vec::new();
    if let Some((record, refresh, state_locations)) = record {
        refresh_at = refresh
            .map(|deadline| Instant::now() + (deadline - Utc::now()).to_std().unwrap_or_default());
        locations = state_locations;
        let output = serde_json::to_string(&record)
            .map_err(|error| ControlError::Invalid(error.to_string()))?;
        if write_output(&[(Stream::Stdout, &format!("{output}\n"))]).is_some() {
            return Err(ControlError::Invalid(format!(
                "Snapshot {} was published, but result delivery failed",
                record.snapshot_id
            )));
        }
    }
    Ok(Some((published, refresh_at, locations)))
}

/// The child is retained by its owning task while this exchange can be cancelled.
pub(crate) async fn exchange_location_generation(
    compiler: &mut LocationCompiler,
    generation: i64,
) -> Result<Option<LocationPublication>, ControlError> {
    compiler
        .stdin
        .write_all(format!("{generation}\n").as_bytes())
        .await?;
    compiler.stdin.flush().await?;
    let mut line = String::new();
    if compiler.stdout.read_line(&mut line).await? == 0 {
        return Err(ControlError::Invalid(
            "location interpreter exited before replying".into(),
        ));
    }
    serde_json::from_str(&line).map_err(|_| {
        ControlError::Invalid("location interpreter returned an invalid publication record".into())
    })
}

/// The caller retains and reaps the child while a cancellable bounded exchange owns its pipes.
pub(crate) fn location_result<T: serde::de::DeserializeOwned>(
    (options, script): (&LocationServe, &Path),
    (request, maximum_bytes): (Vec<u8>, u64),
) -> Result<(Child, impl Future<Output = Result<T, ControlError>>), ControlError> {
    let mut child = location_runtime_child(options, script)?;
    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| ControlError::Invalid("location input is unavailable".into()))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| ControlError::Invalid("location output is unavailable".into()))?;
    Ok((child, async move {
        stdin.write_all(&request).await?;
        stdin.write_all(b"\n").await?;
        stdin.shutdown().await?;
        drop(stdin);
        let mut bytes: Vec<u8> = Vec::new();
        stdout
            .take(maximum_bytes + 1)
            .read_to_end(&mut bytes)
            .await?;
        if bytes.len() as u64 > maximum_bytes {
            return Err(ControlError::Invalid(
                "location reply exceeds its byte limit".into(),
            ));
        }
        serde_json::from_slice(&bytes)
            .map_err(|_| ControlError::Invalid("invalid location reply".into()))
    }))
}

pub(crate) fn metadata_sync(
    (locations, generation): (Vec<WarehouseStateLocation>, i64),
    reconcile_seconds: u32,
) -> LocationMetadataSync {
    LocationMetadataSync {
        locations,
        generation,
        index: 0,
        scan_id: Uuid::now_v7(),
        source_after: None,
        observed_since: None,
        watermark_loaded: false,
        statistics: None,
        reconcile: true,
        reconcile_at: Instant::now(),
        reconcile_seconds,
        limit: MAX_HISTORY_RECORDS,
        pending: None,
        next_at: Instant::now(),
    }
}

pub(crate) fn history_request(state: &LocationMetadataSync) -> Result<Vec<u8>, ControlError> {
    if state.index == 0 && state.source_after.is_none() && state.limit == MAX_HISTORY_RECORDS {
        drop(write_output(&[(
            Stream::Stderr,
            "Reading warehouse history for automatic synchronization...\n",
        )]));
    }
    let kind = state.kind().as_str();
    let request = if state.kind() == WarehouseHistoryKind::Statistics {
        json!({(kind): state.statistics.as_ref().ok_or_else(|| ControlError::Invalid("statistics eligibility page missing".into()))?})
    } else {
        json!({(kind): {
            "location":state.location(),"after":state.source_after,"observed_since":state.observed_since,"limit":state.limit
        }})
    };
    serde_json::to_vec(&request).map_err(|error| ControlError::Invalid(error.to_string()))
}

pub(crate) async fn statistics_page(
    (client, target): (&LocationWorkClient, &str),
    (state, poll): (&LocationMetadataSync, &LocationWorkPoll),
) -> Result<StatisticsRead, ClientError> {
    let after = state
        .source_after
        .as_ref()
        .and_then(|values| values.first())
        .and_then(|value| value.as_deref())
        .map(Uuid::parse_str)
        .transpose()
        .map_err(|_| ClientError::Configuration("invalid statistics eligibility cursor"))?;
    client
        .statistics_due(
            target,
            &StatisticsDueRead {
                session_id: poll.session_id,
                generation: state.generation,
                location: state.location().clone(),
                after,
                limit: state.limit,
            },
        )
        .await
}

/// Freeze one bounded page; a full scan's last page also removes rows the scan did not deliver.
pub(crate) fn freeze_history_page(
    mut state: LocationMetadataSync,
    page: WarehouseHistoryPage,
    poll: &LocationWorkPoll,
) -> Result<LocationMetadataSync, ControlError> {
    let batch = MetadataBatch {
        kind: state.kind(),
        session_id: poll.session_id,
        generation: state.generation,
        records: page
            .records
            .iter()
            .map(serde_json::to_value)
            .collect::<Result<_, _>>()
            .map_err(|error| ControlError::Invalid(error.to_string()))?,
        scan: state.reconciling().then(|| MetadataScan {
            scan_id: state.scan_id,
            location: state.location().clone(),
            complete: page.next_after.is_none(),
        }),
    };
    if serde_json::to_vec(&batch)
        .map_err(|error| ControlError::Invalid(error.to_string()))?
        .len()
        > BODY_BYTES
    {
        state.statistics = None;
        if state.limit > 1 {
            state.limit = (state.limit / 2).max(1);
        } else {
            state.next_at = Instant::now() + Duration::from_secs(METADATA_POLL_SECONDS);
            drop(write_output(&[(
                Stream::Stderr,
                "Warehouse history row exceeds the request bound; synchronization incomplete, no stale rows removed.\n",
            )]));
        }
    } else {
        state.pending = Some(LocationMetadataPage {
            batch,
            next_source_after: page.next_after,
        });
    }
    Ok(state)
}

pub(crate) fn acknowledge_metadata_page(mut state: LocationMetadataSync) -> LocationMetadataSync {
    let Some(pending) = state.pending.take() else {
        return state;
    };
    state.source_after = pending.next_source_after;
    if state.kind() == WarehouseHistoryKind::Statistics {
        state.statistics = None;
    }
    if state.source_after.is_none() {
        state.index += 1;
        state.scan_id = Uuid::now_v7();
        state.observed_since = None;
        state.watermark_loaded = false;
        state.limit = MAX_HISTORY_RECORDS;
        if state.index == state.locations.len() * WarehouseHistoryKind::ALL.len() {
            state.index = 0;
            if state.reconcile {
                state.reconcile_at =
                    Instant::now() + Duration::from_secs(u64::from(state.reconcile_seconds));
            }
            state.next_at = Instant::now() + Duration::from_secs(METADATA_POLL_SECONDS);
            drop(write_output(&[(
                Stream::Stderr,
                "Warehouse history synchronization completed.\n",
            )]));
        }
    }
    state
}

/// Operational cost is elapsed IO time, not an estimate of warehouse billing.
pub(crate) async fn metadata_health(
    state: &LocationMetadataSync,
    (client, poll): (&LocationWorkClient, &LocationWorkPoll),
    (started, failure): (Instant, Option<WarehouseSyncFailure>),
) {
    let report = LocationHealthReport {
        event_id: Uuid::now_v7(),
        session_id: poll.session_id,
        target: poll.target.clone(),
        health: LocationHealth::WarehouseSync {
            generation: state.generation,
            location: state.location().clone(),
            table: state.kind(),
            failure,
            records_sent: state
                .pending
                .as_ref()
                .map_or(0, |page| page.batch.records.len()) as u64,
            elapsed_millis: started.elapsed().as_millis() as u64,
        },
    };
    if let Err(error) = client.report_health(&report).await {
        drop(write_output(&[(
            Stream::Stderr,
            &format!("Warehouse synchronization health report failed: {error}\n"),
        )]));
    }
}

/// Health IO remains native even while a user-code child holds its interpreter's GIL.
pub(crate) async fn report_location_health(
    options: &LocationServe,
    bearer: &str,
    session_id: Uuid,
    health: LocationHealth,
) {
    let report = LocationHealthReport {
        event_id: Uuid::now_v7(),
        session_id,
        target: options.target.clone(),
        health,
    };
    if let Err(error) =
        report_remote_location_health((&options.server_url, bearer), options.location_id, &report)
            .await
    {
        drop(write_output(&[(
            Stream::Stderr,
            &format!("Location health report failed: {error}\n"),
        )]));
    }
}

pub(crate) async fn location_heartbeats(
    options: &LocationServe,
    bearer: &str,
    session_id: Uuid,
) -> Result<(), ControlError> {
    let mut clock = tokio::time::interval(Duration::from_secs(30));
    clock.set_missed_tick_behavior(MissedTickBehavior::Skip);
    loop {
        clock.tick().await;
        report_location_health(options, bearer, session_id, LocationHealth::Heartbeat {}).await;
    }
}

pub(crate) fn location_options(namespace: &Namespace) -> Result<LocationServe, ControlError> {
    if namespace
        .get("location_generation")
        .is_some_and(|value| *value != Value::None)
    {
        return Err(ControlError::Invalid(
            "serve derives --generation from the server; do not supply it".into(),
        ));
    }
    let project_dir = text(namespace, "project_dir")
        .map_or_else(std::env::current_dir, |directory| {
            std::fs::canonicalize(directory)
        })?;
    if !project_dir.is_dir() {
        return Err(ControlError::Invalid("project directory must exist".into()));
    }
    let location_id = text(namespace, "location_id")
        .unwrap_or_default()
        .parse()
        .map_err(|_| ControlError::Invalid("location ID must be a UUID".into()))?;
    let target = text(namespace, "target").unwrap_or_default();
    if target.trim().is_empty() {
        return Err(ControlError::Invalid("target must not be blank".into()));
    }
    let metadata_reconcile_seconds = std::env::var("SQLBUILD_METADATA_RECONCILE_SECONDS")
        .map_or(Ok(METADATA_RECONCILE_SECONDS), |value| {
            value
                .parse::<std::num::NonZeroU32>()
                .map(std::num::NonZeroU32::get)
        })
        .map_err(|_| {
            ControlError::Invalid(
                "SQLBUILD_METADATA_RECONCILE_SECONDS must be a positive integer".into(),
            )
        })?;
    Ok(LocationServe {
        project_dir,
        server_url: text(namespace, "location_server_url").unwrap_or_default(),
        location_id,
        target,
        image_id: text(namespace, "location_image_id"),
        metadata_reconcile_seconds,
    })
}

/// Reconcile from the server after restarts or lost acknowledgements; transient outages retry.
pub(crate) async fn location_generation(
    options: &LocationServe,
    bearer: &str,
) -> Result<Option<i64>, ControlError> {
    match next_remote_generation(
        &options.server_url,
        bearer,
        options.location_id,
        &options.target,
    )
    .await
    {
        Ok(generation) => Ok(Some(generation)),
        Err(error) => {
            if matches!(&error, ClientError::Configuration(_))
                || matches!(&error, ClientError::Rejected(status) if matches!(status.as_u16(), 401 | 403))
            {
                return Err(ControlError::Invalid(error.to_string()));
            }
            drop(write_output(&[(
                Stream::Stderr,
                &format!(
                    "Location generation lookup failed; retaining last good snapshot and retrying: {error}\n"
                ),
            )]));
            Ok(None)
        }
    }
}

/// The installed interpreter owns user imports; the supervisor never imports Python.
pub(crate) fn location_runtime_child(options: &LocationServe, script: &Path) -> io::Result<Child> {
    let mut command = location_child(options, script);
    command
        .args([
            "location",
            "runtime",
            "--server",
            &options.server_url,
            "--location-id",
        ])
        .arg(options.location_id.to_string())
        .args(["--target", &options.target]);
    if let Some(image) = &options.image_id {
        command.args(["--image-id", image]);
    }
    command.stdin(Stdio::piped()).stdout(Stdio::piped()).spawn()
}

pub(crate) async fn check_published_location(
    options: &LocationServe,
    script: &Path,
    (bearer, session_id, generation): (&str, Uuid, i64),
    stopped: impl Future<Output = Result<(), ControlError>>,
) -> Result<Option<Vec<PathBuf>>, ControlError> {
    drop(write_output(&[(
        Stream::Stderr,
        "Checking location connection with sqb debug...\n",
    )]));
    tokio::pin!(stopped);
    let Some((connected, files)) =
        check_location_connection(options, script, stopped.as_mut()).await?
    else {
        return Ok(None);
    };
    tokio::select! {
        result = &mut stopped => { result?; return Ok(None); },
        () = report_location_health(options, bearer, session_id, LocationHealth::Connection { generation, healthy: connected }) => {},
    }
    drop(write_output(&[(
        Stream::Stderr,
        if connected {
            "Code location ready; connection check passed; watching for changes.\n"
        } else {
            "Code location published; connection check failed; watching for changes.\n"
        },
    )]));
    Ok(Some(files))
}

/// Capture the ordinary debug check's exact local file paths without contaminating publication stdout.
pub(crate) async fn check_location_connection(
    options: &LocationServe,
    script: &Path,
    stopped: impl Future<Output = Result<(), ControlError>>,
) -> Result<Option<(bool, Vec<PathBuf>)>, ControlError> {
    let mut child = location_child(options, script)
        .args(["debug", "--target", &options.target, "--json"])
        .stdout(Stdio::piped())
        .spawn()?;
    let mut stdout = child
        .stdout
        .take()
        .ok_or_else(|| ControlError::Invalid("location debug output is unavailable".into()))?;
    let mut output: Vec<u8> = Vec::new();
    tokio::pin!(stopped);
    let connected = tokio::select! {
        result = &mut stopped => {
            child.kill().await?;
            result?;
            return Ok(None);
        },
        result = async { tokio::try_join!(child.wait(), stdout.read_to_end(&mut output)) } => result?.0.success(),
    };
    let files: Vec<String> = match serde_json::from_slice::<serde_json::Value>(&output)
        .and_then(|report| serde_json::from_value(report["local_files"].clone()))
    {
        Ok(files) => files,
        Err(_) => {
            drop(write_output(&[(
                Stream::Stderr,
                "Location connection check did not produce a valid report; retaining the published snapshot.\n",
            )]));
            return Ok(Some((false, Vec::new())));
        }
    };
    Ok(Some((
        connected,
        location_database_files(&options.project_dir, files),
    )))
}

fn location_child(options: &LocationServe, script: &Path) -> Command {
    let mut command = Command::new(script);
    command.arg("--project-dir").arg(&options.project_dir);
    if terminal_columns().is_err() {
        command.env(COLUMNS_VARIABLE, FALLBACK_COLUMNS.to_string());
    }
    command
        .stdin(Stdio::null())
        .stderr(Stdio::inherit())
        .kill_on_drop(true);
    command
}

/// Health production and delivery remain native and independent of scheduler leadership.
pub(crate) fn start_platform_roles(
    tasks: &mut JoinSet<Result<(), ControlError>>,
    (store, shutdown): (&ControlStore, &watch::Receiver<bool>),
    retention: (AuditRetention, StatisticsRetention, SnapshotRetention),
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
        let sink_store = store.clone();
        let sink_shutdown = shutdown.clone();
        tasks.spawn(async move { run_webhook(&sink_store, sink, sink_shutdown).await });
        drop(write_output(&[(
            Stream::Stderr,
            &format!("{label} webhook started\n"),
        )]));
    }
    Ok(())
}

/// Dispatch maintenance actions without implicitly initializing or upgrading their store.
pub(crate) async fn run_control_action(
    namespace: &Namespace,
    command: &str,
) -> Result<Option<serde_json::Value>, ControlError> {
    if command == ADMIN_COMMAND {
        return run_administration(namespace).await.map(Some);
    }
    run_control(namespace, command == SCHEDULER_COMMAND)
        .await
        .map(|()| None)
}

/// Keep exclusive offline recovery outside the online store-opening and migration path.
pub(crate) async fn run_administration(
    namespace: &Namespace,
) -> Result<serde_json::Value, ControlError> {
    if text(namespace, "admin_command").as_deref() != Some("recover-user") {
        return run_admin(namespace).await;
    }
    let principal_id = Uuid::parse_str(&text(namespace, "admin_subject").unwrap_or_default())
        .map_err(|_| ControlError::Invalid("a valid subject UUID is required".into()))?;
    let reason = text(namespace, "admin_reason").unwrap_or_default();
    if reason.trim().is_empty() {
        return Err(ControlError::Invalid(
            "offline recovery requires --reason".into(),
        ));
    }
    let url = control_database(namespace)?;
    drop(write_output(&[(
        Stream::Stderr,
        "Acquiring exclusive offline control-store access...\n",
    )]));
    let recovery = begin_offline_recovery(&url).await?;
    let context = AuditContext {
        actor: "local-control-store-owner".into(),
        granting_role: "control-store-owner".into(),
        request_id: Uuid::now_v7(),
        ip_address: None,
    };
    recover_administrator(recovery, &context, (principal_id, &reason)).await?;
    Ok(serde_json::json!({"principal_id":principal_id,"role":"admin","recovered":true}))
}

/// Open and migrate the privileged store, reporting progress without contaminating JSON stdout.
pub(crate) async fn open_control_store(
    namespace: &Namespace,
) -> Result<ControlStore, ControlError> {
    let url = control_database(namespace)?;
    drop(write_output(&[(
        Stream::Stderr,
        "Opening control store...\n",
    )]));
    let store = open_store(&url).await?;
    migrate_store(&store).await?;
    drop(write_output(&[(Stream::Stderr, "Control store ready\n")]));
    Ok(store)
}

/// Decode privileged CLI configuration before handing process ownership to the workers domain.
pub(crate) fn open_control_workers(
    namespace: &Namespace,
) -> Result<(Workers, Vec<DockerImageSource>), ControlError> {
    let workers = open_workers(
        text(namespace, "control_launchers").as_deref(),
        text(namespace, "control_database_env")
            .unwrap_or_else(|| "SQLBUILD_CONTROL_DATABASE_URL".into()),
        !flag(namespace, "control_no_scheduler"),
    )?;
    let images = docker_image_sources(&workers);
    Ok((workers, images))
}

pub(crate) async fn shutdown_signal() -> Result<(), ControlError> {
    #[cfg(unix)]
    {
        let mut terminate =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
        tokio::select! {
            result = tokio::signal::ctrl_c() => result?,
            _ = terminate.recv() => {},
        }
    }
    #[cfg(not(unix))]
    tokio::signal::ctrl_c().await?;
    Ok(())
}

/// Stop sibling roles on a signal or role failure, then drain their shutdown results.
pub(crate) async fn supervise_control_roles(
    mut tasks: JoinSet<Result<(), ControlError>>,
    stop: watch::Sender<bool>,
) -> Result<(), ControlError> {
    let mut result = tokio::select! {
        result = tasks.join_next() => match result {
            Some(result) => result.map_err(ControlError::from).and_then(std::convert::identity),
            None => Ok(()),
        },
        result = shutdown_signal() => result,
    };
    stop.send_replace(true);
    while let Some(joined) = tasks.join_next().await {
        let joined = joined
            .map_err(ControlError::from)
            .and_then(std::convert::identity);
        result = result.and(joined);
    }
    result
}

/// The capture Python's `dispatch_with_compute_logs` opens; a failure only loses the log.
fn open_capture(
    invocation: &NativeInvocation,
    project: &ProjectLocation,
    started_at: SystemTime,
) -> io::Result<OpenCapture> {
    let retention: i64 = i64::try_from(DEFAULT_RETENTION_COUNT).unwrap_or(i64::MAX);
    let root: PathBuf = open_log_root(&project.given, None, Some(retention))
        .map_err(|error| io::Error::other(format!("{error:?}")))?;
    start_capture(CaptureStart {
        root,
        command: invocation
            .command_name
            .clone()
            .unwrap_or_else(|| UNKNOWN_COMMAND.to_owned()),
        project_dir: project.resolved.display().to_string(),
        target: invocation.target.clone(),
        started_at,
    })
}

/// Close the capture the way Python's does once the command's output met its reader.
fn close_capture(capture: OpenCapture, mut end: CaptureEnd, lost: Option<&LostStdout>) {
    match lost {
        None => drop(finish_capture(capture, &end)),
        Some(LostStdout::Raised(_)) => {
            end.stdout.clear();
            end.exit_code = i64::from(UNCAUGHT_EXCEPTION_STATUS);
            drop(finish_capture(capture, &end));
        }
        Some(LostStdout::AtExit(_) | LostStdout::Dropped) => drop(abandon_capture(capture, &end)),
    }
}

/// Print an observed command's output after the skill notice, retaining both in its capture.
fn finish_observed(
    invocation: &NativeInvocation,
    (project, package): (&ProjectLocation, &InstalledPackage),
    started_at: SystemTime,
    output: CommandOutput,
) -> i32 {
    let notice: String = if invocation.maintains_skills {
        skill_freshness_notice(&project.given, package)
    } else {
        String::new()
    };
    let capture: io::Result<OpenCapture> = open_capture(invocation, project, started_at);
    let stderr: String = notice + &output.stderr;
    let lost: Option<LostStdout> = write_output(&[
        (Stream::Stderr, stderr.as_str()),
        (Stream::Stdout, output.stdout.as_str()),
    ]);
    if let Ok(capture) = capture {
        close_capture(
            capture,
            CaptureEnd {
                stdout: output.stdout.into_bytes(),
                stderr: stderr.into_bytes(),
                exit_code: i64::from(output.exit_code),
            },
            lost.as_ref(),
        );
    }
    finish_lost_stdout(lost.as_ref(), output.exit_code)
}

/// What an observed command prints, unless it needs Python after all.
fn observed_output(
    command: &NativeCommand,
    project: &ProjectLocation,
    package: &InstalledPackage,
    error_color: bool,
) -> Result<CommandOutput, PythonNeeded> {
    if !has_no_runtime_extensions(&project.given) {
        return Err(PythonNeeded::Project);
    }
    match command {
        NativeCommand::Nothing => Ok(CommandOutput::default()),
        NativeCommand::Skills(options) => run_skills(&project.resolved, package, options),
        NativeCommand::Rules(options) => run_rules(&project.resolved, options),
        NativeCommand::Cost(options) => run_cost(&project.given, options, error_color),
        NativeCommand::Init => Err(PythonNeeded::Command),
    }
}

/// Run a parsed Python-free command; an error hands it to Python before any side effect.
pub(crate) fn run_command(
    invocation: &NativeInvocation,
    python_cli: &Path,
    error_color: bool,
) -> Result<i32, PythonNeeded> {
    let package: InstalledPackage = installed_package(python_cli)?;
    let project: ProjectLocation = project_location(invocation.project_dir.as_deref())?;
    if invocation.creates_project {
        let output: CommandOutput = run_init(&project.resolved, &package, error_color)?;
        let lost: Option<LostStdout> = write_output(&[
            (Stream::Stderr, output.stderr.as_str()),
            (Stream::Stdout, output.stdout.as_str()),
        ]);
        return Ok(finish_lost_stdout(lost.as_ref(), output.exit_code));
    }
    let started_at: SystemTime = SystemTime::now();
    let output: CommandOutput =
        observed_output(&invocation.command, &project, &package, error_color)?;
    Ok(finish_observed(
        invocation,
        (&project, &package),
        started_at,
        output,
    ))
}
