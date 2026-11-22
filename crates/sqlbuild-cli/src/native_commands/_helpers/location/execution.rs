//! Running a Python-free command: its checks, then its output with the observed services.

use crate::adhoc::main::adhoc_planner::adhoc_planner;
use crate::argv::models::{Namespace, Stream, Value};
use crate::compute_logs::constants::DEFAULT_RETENTION_COUNT;
use crate::compute_logs::main::abandon_capture::abandon_capture;
use crate::compute_logs::main::finish_capture::finish_capture;
use crate::compute_logs::main::open_log_root::open_log_root;
use crate::compute_logs::main::start_capture::start_capture;
use crate::compute_logs::models::{CaptureEnd, CaptureStart, OpenCapture};
use crate::dev_screen::main::terminal_size::terminal_size;
use crate::dev_screen::models::ScreenSetup;
use crate::dev_screen::models::{DevStep, ScreenMessage};
use crate::dev_screen::types::DevScreenParts;
use crate::dev_session::main::run_dev_session::run_dev_session;
use crate::dev_session::models::{DevProject, LocationPipes};
use crate::dev_stop::main::stop_switch::stop_switch;
use crate::dev_stop::models::DevEnd;
use crate::native_commands::_helpers::command::decision::{
    control_database, control_database_variable, flag, text,
};
use crate::native_commands::_helpers::command::init::run_init;
use crate::native_commands::_helpers::command::package::installed_package;
use crate::native_commands::_helpers::command::paths::exclude_location_databases;
use crate::native_commands::_helpers::command::paths::{location_database_files, project_location};
use crate::native_commands::_helpers::command::project_facts::has_no_runtime_extensions;
use crate::native_commands::_helpers::command::project_facts::{
    development_project_name, project_name,
};
use crate::native_commands::_helpers::command::rules::run_rules;
use crate::native_commands::_helpers::command::skills::{run_skills, skill_freshness_notice};
use crate::native_commands::_helpers::command::terminal::share_terminal;
use crate::native_commands::_helpers::command::terminal::{
    finish_lost_stdout, terminal_columns, write_output,
};
use crate::native_commands::_helpers::command::terminal::{live_screen, screen_colour};
use crate::native_commands::_helpers::location::dev_cancel::DevRuns;
use crate::native_commands::_helpers::location::dev_exposure::{
    allowed_hosts, connect_address, exposure, listens_beyond_loopback,
};
use crate::native_commands::_helpers::location::dev_supervision::{
    DevRoles, supervise_development_roles,
};
use crate::native_commands::_helpers::location::dynamic_resume::resume_expired_holds;
use crate::native_commands::_helpers::location::env_reload::env_file_summary;
use crate::native_commands::_helpers::location::provider_secrets::report_provider_secrets;
use crate::native_commands::_helpers::location::sync_report::{
    metadata_health, read_timeout_seconds, rows_text, scan_line,
};
use crate::native_commands::constants::PLAN_NOT_PRODUCED;
use crate::native_commands::constants::{
    ADMIN_COMMAND, CHANGED_PATHS_SHOWN, COLUMNS_VARIABLE, FALLBACK_COLUMNS, FALLBACK_ROWS,
    METADATA_OVERLAP_SECONDS, METADATA_POLL_SECONDS, METADATA_READER_CLOSE_SECONDS,
    METADATA_RECONCILE_SECONDS, METADATA_REPLY_BYTES, METADATA_SAFETY_SYNC_SECONDS,
    METADATA_SYNC_CHECK_SECONDS, SAFETY_SYNC_ENV, SCHEDULER_COMMAND, UNCAUGHT_EXCEPTION_STATUS,
    UNKNOWN_COMMAND,
};
use crate::native_commands::constants::{OPENING_STORE, STORE_READY};
use crate::native_commands::errors::PythonNeeded;
use crate::native_commands::main::_run_admin::run_admin;
use crate::native_commands::main::_run_control::run_control;
use crate::native_commands::main::_run_cost::run_cost;
use crate::native_commands::models::AdminDone;
use crate::native_commands::models::{
    CommandOutput, DevControlState, InstalledPackage, LocationCompileRequest, LocationCompiler,
    LocationMetadataPage, LocationMetadataSync, LocationServe, LocationWatch, LostStdout,
    MetadataReply, NativeCommand, NativeInvocation, ProjectLocation, ScanToken,
};
use crate::native_commands::models::{
    LocationLoad, LocationRuntimeRequest, ReadFailure, SyncProgress,
};
use crate::native_commands::types::LoadOutcome;
use crate::native_commands::types::{NextGeneration, Settled};
use crate::server_config::models::ServerSection;
use crate::workers::main::docker_image_sources::docker_image_sources;
use crate::workers::main::open_development_workers::open_development_workers;
use crate::workers::main::open_workers::open_workers;
use crate::workers::main::terminate_child::terminate_child;
use crate::workers::models::{Launchers, Workers};
use chrono::Utc;
use serde_json::json;
use sqlbuild_cache::project_snapshot::models::PathStamp;
use sqlbuild_config::project::main::load_control_config::load_control_config;
use sqlbuild_config::project::main::project_config_paths::project_config_paths;
use sqlbuild_config::project::main::sync_interval_seconds::sync_interval_seconds;
use sqlbuild_config::project::models::ControlConfig;
use sqlbuild_core::cli_text::models::CliStyle;
use sqlbuild_core::process_environment::constants::RUN_TOKEN_VARIABLE;
use sqlbuild_core::process_environment::main::apply_environment_overlay::apply_environment_overlay;
use sqlbuild_core::redaction::main::redact_error_text::redact_error_text;
use sqlbuild_core::screen_text::models::{DevHeader, ScreenFacts};
use sqlbuild_orchestration::api::constants::BODY_BYTES;
use sqlbuild_orchestration::api::errors::ClientError;
use sqlbuild_orchestration::api::main::serve_api::serve_api;
use sqlbuild_orchestration::api::models::{
    ApiAuthentication, DevelopmentScope, HumanLogin, ServerFacts,
};
use sqlbuild_orchestration::credentials::main::identities::development_person::development_person;
use sqlbuild_orchestration::credentials::main::locations::development_location::development_location;
use sqlbuild_orchestration::credentials::models::AuditContext;
use sqlbuild_orchestration::declarations::main::pause_development_triggers::pause_development_triggers;
use sqlbuild_orchestration::dev_events::constants::{DEV_EVENTS_ON, DEV_EVENTS_VARIABLE};
use sqlbuild_orchestration::dev_events::main::dev_event_line::dev_event_line;
use sqlbuild_orchestration::dev_events::main::dev_events_enabled::dev_events_enabled;
use sqlbuild_orchestration::dev_events::main::emit_dev_event::emit_dev_event;
use sqlbuild_orchestration::dev_events::main::parse_dev_control::parse_dev_control;
use sqlbuild_orchestration::dev_events::main::parse_dev_event::parse_dev_event;
use sqlbuild_orchestration::dev_events::models::{DevControl, DevEvent};
use sqlbuild_orchestration::platform::constants::PLATFORM_WEBHOOK_VARIABLE;
use sqlbuild_orchestration::platform::main::health::report_remote_location_health::report_remote_location_health;
use sqlbuild_orchestration::platform::main::installation::run_maintenance::run_maintenance;
use sqlbuild_orchestration::platform::models::ConnectionCheckTrigger;
use sqlbuild_orchestration::platform::models::LocationRuleFailure;
use sqlbuild_orchestration::platform::models::PublishedGeneration;
use sqlbuild_orchestration::platform::models::{
    LocationHealth, LocationHealthReport, LocationWorkClient, LocationWorkPoll, RetentionPolicies,
    WarehouseSyncFailure,
};
use sqlbuild_orchestration::run_output::main::open_output_store::open_output_store;
use sqlbuild_orchestration::run_output::models::OutputSettings;
use sqlbuild_orchestration::runs::types::RunHandoff;
use sqlbuild_orchestration::scheduler::main::run_scheduler::run_scheduler;
use sqlbuild_orchestration::scheduler::models::SchedulerState;
use sqlbuild_orchestration::snapshots::main::publication::next_remote_generation::next_remote_generation;
use sqlbuild_orchestration::store::errors::ControlError;
use sqlbuild_orchestration::store::main::migrate_store::migrate_store;
use sqlbuild_orchestration::store::main::open_store::open_store;
use sqlbuild_orchestration::store::main::set_release_version::set_release_version;
use sqlbuild_orchestration::store::models::ControlStore;
use sqlbuild_orchestration::warehouse_metadata::constants::MAX_HISTORY_PAGE_RECORDS;
use sqlbuild_orchestration::warehouse_metadata::constants::MAX_HISTORY_RECORDS;
use sqlbuild_orchestration::warehouse_metadata::models::{
    MetadataBatch, MetadataScan, StatisticsDueRead, StatisticsRead, WarehouseHistoryFailure,
    WarehouseHistoryKind, WarehouseHistoryPage, WarehouseStateLocation, WarehouseSyncDueRead,
    WarehouseWatermarkRead,
};
use sqlbuild_orchestration::webhooks::constants::SECURITY_AUDIT_WEBHOOK_VARIABLE;
use sqlbuild_orchestration::webhooks::main::{
    connect_webhook::connect_webhook, run_webhook::run_webhook,
};
use sqlbuild_orchestration::webhooks::models::{Webhook, WebhookSource};
use std::collections::{BTreeMap, HashMap};
use std::future::Future;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::{Duration, SystemTime};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::mpsc::UnboundedSender;
use tokio::sync::{mpsc, oneshot, watch};
use tokio::task::JoinSet;
use tokio::time::{Instant, Interval, MissedTickBehavior};
use uuid::Uuid;

/// Own the API, scheduler and outbound location as one development process tree, drawn by `screen`, until a stop or a role ends it.
pub(crate) async fn run_development_roles(
    (store, scope): (ControlStore, DevelopmentScope),
    (project, executable): (PathBuf, PathBuf),
    listener: tokio::net::TcpListener,
    (scheduler, (messages, actions, keys)): (bool, DevScreenParts),
) -> Result<DevEnd, ControlError> {
    let listening = listener.local_addr()?;
    let address = connect_address(listening);
    let url = format!("http://{address}");
    let opened_to: Option<String> = exposure(listening, &scope);
    let location_id = scope.location_id;
    let target = scope.target.clone();
    let workers = open_development_workers(
        (location_id, target.clone()),
        project.clone(),
        executable.clone(),
        url.clone(),
    )?;
    let (stop, shutdown) = watch::channel(false);
    let mut roles: DevRoles = DevRoles::default();
    drop(messages.send(ScreenMessage::Started {
        step: DevStep::Api,
        detail: String::new(),
    }));
    let api_ready = messages.clone();
    roles.spawn(
        Some("UI server"),
        serve_api(
            store.clone(),
            listener,
            (
                ApiAuthentication {
                    login: HumanLogin::Development(scope),
                    ..ApiAuthentication::default()
                },
                (Vec::new(), adhoc_planner()),
                ServerFacts {
                    version: env!("CARGO_PKG_VERSION").into(),
                    scheduler,
                    output: Some(open_output_store(&OutputSettings::default(), &store)?),
                    ..ServerFacts::default()
                },
            ),
            (
                move || {
                    drop(api_ready.send(ScreenMessage::Finished {
                        step: DevStep::Api,
                        detail: address.to_string(),
                    }));
                    if let Some(opened_to) = opened_to {
                        drop(api_ready.send(ScreenMessage::Exposed(opened_to)));
                    }
                },
                development_stopped(shutdown.clone()),
            ),
        ),
    );
    start_platform_roles(&mut roles.tasks, (&store, &shutdown), Default::default())?;
    let (location, pipes) = development_location_role(
        (&project, &executable),
        (location_id, &target),
        (&url, &store),
        shutdown.clone(),
    )?;
    roles.spawn(Some("code location"), location);
    roles.spawn(
        None,
        run_dev_session(
            (messages.clone(), actions, keys),
            (
                pipes,
                DevProject {
                    executable: executable.clone(),
                    project: project.clone(),
                    target: target.clone(),
                },
            ),
            (store.clone(), location_id, url.clone()),
            shutdown.clone(),
        ),
    );
    drop(messages.send(ScreenMessage::Started {
        step: DevStep::Scheduler,
        detail: String::new(),
    }));
    if scheduler {
        roles.runs = Some(DevRuns {
            store: store.clone(),
            location_id,
            target: target.clone(),
        });
        let scheduler_store = store.clone();
        let origin = Some(url.clone());
        roles.spawn(Some("scheduler"), async move {
            run_scheduler(
                &scheduler_store,
                SchedulerState::new(Uuid::now_v7()).with_origin(origin),
                shutdown,
                workers,
            )
            .await
        });
    }
    drop(
        messages.send(ScreenMessage::Finished {
            step: DevStep::Scheduler,
            detail: if scheduler {
                "triggers start paused"
            } else {
                "off (--no-scheduler)"
            }
            .to_owned(),
        }),
    );
    Ok(supervise_development_roles(roles, stop, &messages).await)
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
) -> Result<
    (
        impl Future<Output = Result<(), ControlError>> + Send + 'static,
        LocationPipes,
    ),
    ControlError,
> {
    let mut command = Command::new(executable);
    apply_environment_overlay(command.as_std_mut());
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
        .env_remove(RUN_TOKEN_VARIABLE)
        .env_remove("SQLBUILD_DEVELOPMENT_CONTROL_FILE")
        .env(DEV_EVENTS_VARIABLE, DEV_EVENTS_ON)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    #[cfg(unix)]
    command.process_group(0);
    if let ControlStore::Sqlite(path) = store {
        command.env(
            "SQLBUILD_DEVELOPMENT_CONTROL_FILE",
            std::env::current_dir()?.join(path),
        );
    }
    let mut child = command.spawn()?;
    let group: Option<u32> = child.id();
    if let Some(group) = group {
        stop_switch().register_group(group);
    }
    let (Some(stdin), Some(stdout), Some(stderr)) =
        (child.stdin.take(), child.stdout.take(), child.stderr.take())
    else {
        return Err(ControlError::Invalid(
            "development code location pipes are unavailable".into(),
        ));
    };
    let pipes = LocationPipes {
        stdin,
        stdout,
        stderr,
    };
    Ok((
        async move {
            let ended: Result<(), ControlError> = tokio::select! {
                result = child.wait() => {
                    let status = result?;
                    Err(ControlError::Invalid(format!("the code location stopped unexpectedly ({status})")))
                },
                () = development_stopped(shutdown) => {
                    stop_switch().terminate_groups();
                    terminate_child(&mut child, false)?;
                    match tokio::time::timeout(Duration::from_secs(10), child.wait()).await {
                        Ok(result) => { result?; },
                        Err(_) => child.kill().await?,
                    }
                    Ok(())
                },
            };
            if let Some(group) = group {
                stop_switch().forget_group(group);
            }
            ended
        },
        pipes,
    ))
}

/// Request one publication without transferring interpreter ownership to a cancellable future.
pub(crate) async fn compile_generation(
    (requests, dev_events): (&mpsc::Sender<LocationRuntimeRequest>, bool),
    generation: i64,
    sources: &[sqlbuild_cache::project_snapshot::models::PathStamp],
    stopped: impl Future<Output = Result<(), ControlError>>,
) -> Result<Option<LoadOutcome>, ControlError> {
    let (reply, publication) = oneshot::channel();
    requests
        .send(LocationRuntimeRequest::Publish(LocationCompileRequest {
            generation,
            python_sources: sources
                .iter()
                .filter(|stamp| stamp.relative_path.ends_with(".py"))
                .cloned()
                .collect(),
            reply,
        }))
        .await
        .map_err(|_| ControlError::Invalid("location compiler stopped".into()))?;
    tokio::pin!(stopped);
    let result = tokio::select! {
        result = &mut stopped => { result?; return Ok(None); },
        result = publication => result.map_err(|_| ControlError::Invalid("location compiler stopped before replying".into()))?,
    };
    let load = match result {
        Ok(load) => load,
        Err(error) => {
            drop(write_output(&[(
                Stream::Stderr,
                &format!(
                    "Location compiler failed; generation {generation} may already be published: {error}\n"
                ),
            )]));
            dev_failure(dev_events, generation, error);
            LocationLoad {
                published: None,
                rules: None,
            }
        }
    };
    let Some(PublishedGeneration {
        record,
        refresh_at: refresh,
        state_locations: locations,
        missing_secrets,
        secrets_needed_by,
    }) = load.published
    else {
        if let Some(rules) = &load.rules {
            dev_failure(dev_events, generation, rule_failure_text(rules));
        }
        return Ok(Some(Err(load.rules)));
    };
    let refresh_at = refresh
        .map(|deadline| Instant::now() + (deadline - Utc::now()).to_std().unwrap_or_default());
    let output: String = if dev_events {
        dev_event_line(&DevEvent::Published {
            generation,
            snapshot_id: record.snapshot_id.to_string(),
            record: serde_json::to_value(&record)
                .map_err(|error| ControlError::Invalid(error.to_string()))?,
        })
    } else {
        let mut output: String = serde_json::to_string(&record)
            .map_err(|error| ControlError::Invalid(error.to_string()))?;
        output.push('\n');
        output
    };
    if write_output(&[(Stream::Stdout, &output)]).is_some() {
        return Err(ControlError::Invalid(format!(
            "Snapshot {} was published, but result delivery failed",
            record.snapshot_id
        )));
    }
    Ok(Some(Ok((
        refresh_at,
        locations,
        (missing_secrets, secrets_needed_by),
    ))))
}

/// Under `sqb dev`, tell the screen a generation failed; the last good generation keeps serving.
fn dev_failure(dev_events: bool, generation: i64, message: String) {
    if dev_events {
        drop(write_output(&[(
            Stream::Stdout,
            &dev_event_line(&DevEvent::Failed {
                generation,
                position: None,
                message,
                code: None,
                help: None,
            }),
        )]));
    }
}

/// Configured rules that failed a load, as one line: how many findings and the first.
fn rule_failure_text(rules: &LocationRuleFailure) -> String {
    let first: String = rules
        .findings
        .first()
        .map(|finding| format!("; first: {} {}", finding.code, finding.message))
        .unwrap_or_default();
    format!(
        "configured rules failed with {} finding(s){first}",
        rules.finding_count
    )
}

/// The child is retained by its owning task while this exchange can be cancelled.
pub(crate) async fn exchange_location_generation(
    compiler: &mut LocationCompiler,
    generation: i64,
) -> Result<LocationLoad, ControlError> {
    compiler
        .stdin
        .write_all(format!("{generation}\n").as_bytes())
        .await?;
    compiler.stdin.flush().await?;
    let mut line = String::new();
    loop {
        line.clear();
        if compiler.stdout.read_line(&mut line).await? == 0 {
            return Err(ControlError::Invalid(
                "location interpreter exited before replying".into(),
            ));
        }
        if parse_dev_event(&line).is_none() {
            break;
        }
        drop(write_output(&[(Stream::Stdout, &line)]));
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
    share_terminal();
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
    (reconcile_seconds, safety_seconds): (u32, u64),
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
        limit: MAX_HISTORY_PAGE_RECORDS,
        statistics_limit: MAX_HISTORY_RECORDS,
        pending: None,
        next_at: Instant::now(),
        idle: true,
        applied: false,
        replan: true,
        cycle_started_at: None,
        server_offset: chrono::Duration::zero(),
        safety_seconds,
        safety_at: Instant::now() + Duration::from_secs(safety_seconds),
        failed_reads: 0,
        halted: false,
        change_tokens: HashMap::new(),
        scan_token: None,
        scan_delivered: false,
        mirror_holds_rows: false,
        reader: None,
        state_history_versions: None,
        progress: SyncProgress::new(),
        reads_wake_compute: false,
        dynamic_quiet_tokens: std::collections::BTreeMap::new(),
    }
}

pub(crate) fn history_request(state: &LocationMetadataSync) -> Result<Vec<u8>, ControlError> {
    if state.index == 0 && state.source_after.is_none() && state.limit == MAX_HISTORY_PAGE_RECORDS {
        drop(write_output(&[(
            Stream::Stderr,
            "Reading warehouse history for automatic synchronization...\n",
        )]));
    }
    let kind = state.kind().as_str();
    let request = if state.kind() == WarehouseHistoryKind::Statistics {
        json!({(kind): state.statistics.as_ref().ok_or_else(|| ControlError::Invalid("statistics eligibility page missing".into()))?})
    } else if state.kind() == WarehouseHistoryKind::Catalog {
        json!({(kind): {
            "location":state.location(),"after":state.source_after,"observed_since":state.observed_since,"limit":state.limit
        }})
    } else {
        let mut read = json!({
            "location":state.location(),"after":state.source_after,"observed_since":state.observed_since,"limit":state.limit,
            "unchanged_token":state.unchanged_token()
        });
        if let Some(versions) = state.newest_versions() {
            read["newest_versions"] = json!(versions);
        }
        json!({(kind): read})
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
                limit: state.statistics_limit,
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
    if state.source_after.is_none() {
        state.scan_token.clone_from(&page.change_token);
        state.scan_delivered = false;
    }
    state.scan_delivered |= !page.records.is_empty();
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
        last_page: page.next_after.is_none(),
    };
    if serde_json::to_vec(&batch)
        .map_err(|error| ControlError::Invalid(error.to_string()))?
        .len()
        > BODY_BYTES
    {
        state.statistics = None;
        let limit = if state.kind() == WarehouseHistoryKind::Statistics {
            &mut state.statistics_limit
        } else {
            &mut state.limit
        };
        if *limit > 1 {
            *limit /= 2;
        } else {
            state.next_at = Instant::now() + Duration::from_secs(METADATA_POLL_SECONDS);
            drop(write_output(&[(
                Stream::Stderr,
                "Warehouse history row exceeds the request bound; synchronization incomplete, no stale rows removed.\n",
            )]));
            location_warning(
                "warehouse sync incomplete",
                "a history row exceeds the request bound".to_owned(),
            );
        }
    } else {
        state.pending = Some(LocationMetadataPage {
            batch,
            next_source_after: page.next_after,
        });
    }
    Ok(state)
}

/// An idle location asks the server, never the warehouse, whether a run ended since it last began.
pub(crate) async fn begin_after_run(
    mut state: LocationMetadataSync,
    (client, poll): (&LocationWorkClient, &LocationWorkPoll),
    target: &str,
) -> (LocationMetadataSync, Result<bool, ClientError>) {
    state.next_at = Instant::now() + Duration::from_secs(METADATA_SYNC_CHECK_SECONDS);
    let request = WarehouseSyncDueRead {
        session_id: poll.session_id,
        generation: state.generation,
    };
    let due = match client.metadata_sync_due(target, &request).await {
        Ok(due) => due,
        Err(error) => return (state, Err(error)),
    };
    let ran = state.sync_due(due.runs_ended_at);
    if ran {
        state.state_history_versions = Some(due.state_history_versions);
        state.begin_sync(due.now);
    }
    (state, Ok(ran))
}

pub(crate) fn acknowledge_metadata_page(mut state: LocationMetadataSync) -> LocationMetadataSync {
    let Some(pending) = state.pending.take() else {
        return state;
    };
    state.progress.scan_rows += pending.batch.records.len() as u64;
    state.source_after = pending.next_source_after;
    if state.kind() == WarehouseHistoryKind::Statistics {
        state.statistics = None;
    }
    if state.source_after.is_none() {
        if state.kind() == WarehouseHistoryKind::Statistics {
            state.statistics_limit = MAX_HISTORY_RECORDS;
        }
        if let Some(count) = state.progress.scan_mirrored {
            let kinds = WarehouseHistoryKind::ALL.len();
            let kind = state.index % kinds;
            state
                .change_tokens
                .iter_mut()
                .filter(|(index, _)| *index % kinds == kind)
                .for_each(|(_, recorded)| recorded.mirrored = Some(count));
        }
        match state.scan_token.take() {
            Some(token) => {
                let recorded = ScanToken {
                    token,
                    reconciled: state.reconciling(),
                    delivered: state.scan_delivered || state.observed_since.is_some(),
                    mirrored: state.progress.scan_mirrored,
                };
                state.change_tokens.insert(state.index, recorded);
            }
            None => {
                state.change_tokens.remove(&state.index);
            }
        }
        state = next_scan(state);
    }
    state
}

/// The mirror's watermark bounds an incremental scan and shows whether the mirror holds rows.
pub(crate) async fn load_watermark(
    mut state: LocationMetadataSync,
    (options, client, poll): (&LocationServe, &LocationWorkClient, &LocationWorkPoll),
    (started, stopped): (Instant, impl Future<Output = Result<(), ControlError>>),
) -> Result<Option<(LocationMetadataSync, Option<ClientError>)>, ControlError> {
    tokio::pin!(stopped);
    if state.index == 0 {
        state.reconcile = Instant::now() >= state.reconcile_at;
    }
    state.mirror_holds_rows = false;
    if (!state.reconcile || state.change_tokens.contains_key(&state.index))
        && !matches!(
            state.kind(),
            WarehouseHistoryKind::Catalog | WarehouseHistoryKind::Statistics
        )
    {
        let request = WarehouseWatermarkRead {
            session_id: poll.session_id,
            generation: state.generation,
            location: state.location().clone(),
            kind: state.kind(),
        };
        let watermark = tokio::select! {
            result = &mut stopped => { result?; return Ok(None); },
            result = client.metadata_watermark(&options.target, &request) => result,
        };
        let watermark = match watermark {
            Ok(watermark) => watermark,
            Err(error) => {
                tokio::select! {
                    result = &mut stopped => { result?; return Ok(None); },
                    () = metadata_health(&state, (client, poll), (started, Some(WarehouseSyncFailure::Watermark))) => {},
                }
                return Ok(Some((state, Some(error))));
            }
        };
        state.mirror_holds_rows = watermark.is_some();
        if !state.reconcile {
            state.observed_since = watermark.and_then(|time| {
                time.checked_sub_signed(chrono::Duration::seconds(METADATA_OVERLAP_SECONDS))
            });
        }
    }
    state.watermark_loaded = true;
    Ok(Some((state, None)))
}

/// Move to the next scan; after the last one the synchronization completes and waits for a run.
pub(crate) fn next_scan(mut state: LocationMetadataSync) -> LocationMetadataSync {
    let scans = state.locations.len() * WarehouseHistoryKind::ALL.len();
    drop(write_output(&[(Stream::Stderr, &scan_line(&state, scans))]));
    state.progress.sync_rows += state.progress.scan_rows;
    state.progress.scan_rows = 0;
    state.progress.scan_unchanged = false;
    state.progress.scan_mirrored = None;
    state.progress.scan_started = Instant::now();
    state.index += 1;
    state.scan_id = state.server_scan_id();
    state.observed_since = None;
    state.watermark_loaded = false;
    state.limit = MAX_HISTORY_PAGE_RECORDS;
    if state.index == state.locations.len() * WarehouseHistoryKind::ALL.len() {
        state.index = 0;
        if state.reconcile {
            state.reconcile_at =
                Instant::now() + Duration::from_secs(u64::from(state.reconcile_seconds));
        }
        state.idle = true;
        state.next_at = Instant::now();
        state.safety_at = Instant::now() + Duration::from_secs(state.safety_seconds);
        drop(write_output(&[(
            Stream::Stderr,
            &format!(
                "Warehouse history synchronization completed: {scans} {}, {} in {:.1} s.\n",
                if scans == 1 { "kind" } else { "kinds" },
                rows_text(state.progress.sync_rows),
                state.progress.sync_started.elapsed().as_secs_f64()
            ),
        )]));
    }
    state
}

/// Under `sqb dev`, a warning the location logs also becomes a screen line, so plain output shows it.
pub(crate) fn location_warning(title: &str, detail: String) {
    drop(emit_dev_event(&DevEvent::Warning {
        title: title.to_owned(),
        detail,
    }));
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
    let environment = match std::env::var(SAFETY_SYNC_ENV) {
        Ok(value) => Some(value),
        Err(std::env::VarError::NotPresent) => None,
        Err(std::env::VarError::NotUnicode(_)) => {
            return Err(ControlError::Invalid(format!(
                "{SAFETY_SYNC_ENV} must be text"
            )));
        }
    };
    let safety_sync = safety_sync_setting(
        text(namespace, "location_sync_at_least_every").as_deref(),
        environment.as_deref(),
        || {
            load_control_config(&project_config_paths(&project_dir)).map_err(|failure| {
                failure.into_config_failure().map_or_else(
                    || "sqlbuild_project.toml could not be read".to_owned(),
                    |failure| failure.render(|_| "sqlbuild_project.toml".to_owned()),
                )
            })
        },
    )
    .map_err(ControlError::Invalid)?;
    Ok(LocationServe {
        safety_sync_seconds: safety_sync.unwrap_or(METADATA_SAFETY_SYNC_SECONDS),
        project_dir,
        server_url: text(namespace, "location_server_url").unwrap_or_default(),
        location_id,
        target,
        image_id: text(namespace, "location_image_id"),
        metadata_reconcile_seconds,
        dev_events: dev_events_enabled(),
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
    if options.dev_events {
        command.env(DEV_EVENTS_VARIABLE, DEV_EVENTS_ON);
    }
    command.stdin(Stdio::piped()).stdout(Stdio::piped()).spawn()
}

/// Check the connection after a publication: the first in a process is its startup check; also returns whether history reads wake billed compute.
pub(crate) async fn check_published_location(
    (options, script, connect): (&LocationServe, &Path, bool),
    ((bearer, session_id), generation, trigger): ((&str, Uuid), i64, ConnectionCheckTrigger),
    stopped: impl Future<Output = Result<(), ControlError>>,
) -> Result<Option<(Vec<PathBuf>, bool)>, ControlError> {
    drop(write_output(&[(
        Stream::Stderr,
        "Checking location connection with sqb debug...\n",
    )]));
    tokio::pin!(stopped);
    let started = Instant::now();
    let Some((connected, wakes_compute, files)) =
        check_location_connection((options, script, connect), stopped.as_mut()).await?
    else {
        return Ok(None);
    };
    let health = LocationHealth::Connection {
        generation,
        healthy: connected,
        trigger: Some(trigger),
        elapsed_millis: Some(u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX)),
    };
    tokio::select! {
        result = &mut stopped => { result?; return Ok(None); },
        () = report_location_health(options, bearer, session_id, health) => {},
    }
    drop(write_output(&[(
        Stream::Stderr,
        if connected {
            "Code location ready; connection check passed; watching for changes.\n"
        } else {
            "Code location published; connection check failed; watching for changes.\n"
        },
    )]));
    Ok(Some((files, wakes_compute)))
}

/// Capture the debug check's local file paths and whether history reads wake billed compute; under `sqb dev` it never connects, so nobody signs in twice.
pub(crate) async fn check_location_connection(
    (options, script, connect): (&LocationServe, &Path, bool),
    stopped: impl Future<Output = Result<(), ControlError>>,
) -> Result<Option<(bool, bool, Vec<PathBuf>)>, ControlError> {
    let mut command = location_child(options, script);
    command.args(["debug", "--target", &options.target, "--json"]);
    if !connect || options.dev_events {
        command.arg("--no-connection");
    }
    let mut child = command.stdout(Stdio::piped()).spawn()?;
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
    let (files, wakes_compute): (Vec<String>, bool) = match serde_json::from_slice::<
        serde_json::Value,
    >(&output)
    .and_then(|report| {
        Ok((
            serde_json::from_value(report["local_files"].clone())?,
            report["history_reads_wake_compute"]
                .as_bool()
                .unwrap_or_default(),
        ))
    }) {
        Ok(report) => report,
        Err(_) => {
            drop(write_output(&[(
                Stream::Stderr,
                "Location connection check did not produce a valid report; retaining the published snapshot.\n",
            )]));
            return Ok(Some((false, false, Vec::new())));
        }
    };
    Ok(Some((
        connect && connected,
        wakes_compute,
        location_database_files(&options.project_dir, files),
    )))
}

pub(crate) fn location_child(options: &LocationServe, script: &Path) -> Command {
    let mut command = Command::new(script);
    apply_environment_overlay(command.as_std_mut());
    command.arg("--project-dir").arg(&options.project_dir);
    if terminal_columns().is_err() {
        command.env(COLUMNS_VARIABLE, FALLBACK_COLUMNS.to_string());
    }
    command
        .env_remove(DEV_EVENTS_VARIABLE)
        .stdin(Stdio::null())
        .stderr(Stdio::inherit())
        .kill_on_drop(true);
    command
}

/// Health production and delivery remain native and independent of scheduler leadership.
pub(crate) fn start_platform_roles(
    tasks: &mut JoinSet<Result<(), ControlError>>,
    (store, shutdown): (&ControlStore, &watch::Receiver<bool>),
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
) -> Result<Option<AdminDone>, ControlError> {
    if command == ADMIN_COMMAND {
        return run_admin(namespace).await.map(Some);
    }
    run_control(namespace, command == SCHEDULER_COMMAND)
        .await
        .map(|()| None)
}

/// Open and migrate the server's store, reporting progress on stderr.
pub(crate) async fn open_control_store(
    namespace: &Namespace,
    server: Option<&ServerSection>,
) -> Result<ControlStore, ControlError> {
    let url = control_database(
        namespace,
        &control_database_variable(namespace, server.unwrap_or(&ServerSection::default())),
    )?;
    drop(write_output(&[(Stream::Stderr, OPENING_STORE)]));
    let store = open_migrated_store(&url).await?;
    drop(write_output(&[(Stream::Stderr, STORE_READY)]));
    Ok(store)
}

/// The store at `url`, created or migrated to this build's schema.
pub(crate) async fn open_migrated_store(url: &str) -> Result<ControlStore, ControlError> {
    set_release_version(env!("CARGO_PKG_VERSION"));
    let store = open_store(url).await?;
    migrate_store(&store).await?;
    Ok(store)
}

/// Hand the configured launchers to the workers domain, which resolves their locations in the store.
pub(crate) async fn open_control_workers(
    (store, namespace): (&ControlStore, &Namespace),
    (server, launchers): (&ServerSection, Launchers),
    scheduler_enabled: bool,
) -> Result<(Workers, RunHandoff), ControlError> {
    let variable = control_database_variable(namespace, server);
    let workers = open_workers(store, launchers, (variable, scheduler_enabled)).await?;
    let images = docker_image_sources(&workers);
    Ok((workers, (images, adhoc_planner())))
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

/// The `{"error": ...}` report a location sends when it has no plan to show.
pub(crate) fn plan_failure(message: &str) -> serde_json::Value {
    json!({"error": {"code": "plan_failed", "message": message, "help": null}})
}

pub(crate) fn open_location_runtime(
    options: &LocationServe,
    script: &Path,
    python_sources: Vec<sqlbuild_cache::project_snapshot::models::PathStamp>,
) -> Result<LocationCompiler, ControlError> {
    let mut child = location_runtime_child(options, script)?;
    let stdin = child
        .stdin
        .take()
        .ok_or_else(|| ControlError::Invalid("location interpreter input is unavailable".into()))?;
    let stdout = child.stdout.take().ok_or_else(|| {
        ControlError::Invalid("location interpreter output is unavailable".into())
    })?;
    Ok(LocationCompiler {
        child,
        stdin,
        stdout: BufReader::new(stdout),
        python_sources,
    })
}

pub(crate) async fn stop_location_runtime(
    runtime: Option<LocationCompiler>,
) -> Result<(), ControlError> {
    if let Some(mut runtime) = runtime {
        if runtime.child.try_wait()?.is_none() {
            runtime.child.kill().await?;
        } else {
            runtime.child.wait().await?;
        }
    }
    Ok(())
}

/// An outer absence means shutdown; an inner error is a failed read, including a page slower than its limit.
pub(crate) async fn read_metadata_page(
    (options, script, page_seconds): (&LocationServe, &Path, u64),
    mut state: LocationMetadataSync,
    stopped: impl Future<Output = Result<(), ControlError>>,
) -> Result<
    Option<(
        LocationMetadataSync,
        Result<WarehouseHistoryPage, ReadFailure>,
    )>,
    ControlError,
> {
    if let Some(statistics) = &state.statistics
        && statistics.names.is_empty()
    {
        let page = WarehouseHistoryPage {
            records: Vec::new(),
            next_after: statistics.next_after.map(|id| vec![Some(id.to_string())]),
            change_token: None,
            unchanged: false,
        };
        return Ok(Some((state, Ok(page))));
    }
    let request = history_request(&state)?;
    let reader = match state.reader.take() {
        Some(reader) => reader,
        None => match open_location_runtime(options, script, Vec::new()) {
            Ok(reader) => reader,
            Err(error) => {
                return Ok(Some((
                    state,
                    Err(ReadFailure::ReaderStart(error.to_string())),
                )));
            }
        },
    };
    let seconds = read_timeout_seconds(&state, page_seconds);
    tokio::pin!(stopped);
    let read = tokio::select! {
        result = &mut stopped => {
            result?;
            return Ok(None);
        },
        result = tokio::time::timeout(
            Duration::from_secs(seconds),
            exchange_metadata_read(reader, request),
        ) => match result {
            Ok((reader, Ok(MetadataReply::Page(page)))) => {
                state.reader = Some(reader);
                state.failed_reads = 0;
                Ok(page)
            }
            Ok((reader, Ok(MetadataReply::Busy))) => {
                state.reader = Some(reader);
                Err(ReadFailure::Busy)
            }
            Ok((reader, Ok(MetadataReply::Failed(failure)))) => {
                state.reader = Some(reader);
                Err(ReadFailure::Read(failure))
            }
            Ok((reader, Err(failure))) => {
                stop_location_runtime(Some(reader)).await?;
                Err(ReadFailure::Read(failure))
            }
            Err(_) => Err(ReadFailure::TimedOut(seconds)),
        },
    };
    Ok(Some((state, read)))
}

/// A completed synchronization resumes expired dynamic holds, then closes its reader, so no warehouse session outlives the sync.
pub(crate) async fn finish_metadata_page(
    state: LocationMetadataSync,
    stopped: impl Future<Output = Result<(), ControlError>>,
) -> Result<Option<LocationMetadataSync>, ControlError> {
    if !state.idle {
        return Ok(Some(state));
    }
    tokio::pin!(stopped);
    let state: LocationMetadataSync = tokio::select! {
        result = &mut stopped => { result?; return Ok(None); },
        state = resume_expired_holds(state) => state,
    };
    Ok(Some(close_metadata_reader(state).await?))
}

/// End of input lets the reader close its warehouse session; one that ignores it is killed.
pub(crate) async fn close_metadata_reader(
    mut state: LocationMetadataSync,
) -> Result<LocationMetadataSync, ControlError> {
    if let Some(LocationCompiler {
        mut child, stdin, ..
    }) = state.reader.take()
    {
        drop(stdin);
        match tokio::time::timeout(
            Duration::from_secs(METADATA_READER_CLOSE_SECONDS),
            child.wait(),
        )
        .await
        {
            Ok(waited) => drop(waited?),
            Err(_) => child.kill().await?,
        }
    }
    Ok(state)
}

/// A reply too long, or a reader gone and likely killed for memory, may fit a smaller page.
async fn exchange_metadata_read(
    mut reader: LocationCompiler,
    request: Vec<u8>,
) -> (
    LocationCompiler,
    Result<MetadataReply, WarehouseHistoryFailure>,
) {
    let failed = |reason: &str, page_size| WarehouseHistoryFailure {
        reason: reason.into(),
        page_size,
    };
    let result = async {
        let written: std::io::Result<()> = async {
            reader.stdin.write_all(&request).await?;
            reader.stdin.write_all(b"\n").await?;
            reader.stdin.flush().await
        }
        .await;
        written.map_err(|error| failed(&error.to_string(), false))?;
        let mut line = String::new();
        let read = (&mut reader.stdout)
            .take(METADATA_REPLY_BYTES + 1)
            .read_line(&mut line)
            .await
            .map_err(|error| failed(&error.to_string(), false))?;
        if read == 0 {
            return Err(failed(
                "location metadata reader exited before replying",
                true,
            ));
        }
        if read as u64 > METADATA_REPLY_BYTES || !line.ends_with('\n') {
            return Err(failed("location reply exceeds its byte limit", true));
        }
        let reply: serde_json::Value =
            serde_json::from_str(&line).map_err(|_| failed("invalid location reply", false))?;
        if reply.get("busy") == Some(&serde_json::Value::Bool(true)) {
            return Ok(MetadataReply::Busy);
        }
        if let Some(failure) = reply.get("failed") {
            return serde_json::from_value(failure.clone())
                .map(MetadataReply::Failed)
                .map_err(|_| failed("invalid location reply", false));
        }
        serde_json::from_value(reply)
            .map(MetadataReply::Page)
            .map_err(|_| failed("invalid location reply", false))
    }
    .await;
    (reader, result)
}

/// Wait for the next tick unless a synchronization's next page is due; `Some` means shutdown.
pub(crate) async fn next_turn(
    clock: &mut Interval,
    (published, watch, metadata): (Option<i64>, &LocationWatch, &Option<LocationMetadataSync>),
    stopped: impl Future<Output = Result<(), ControlError>>,
) -> Option<Result<(), ControlError>> {
    if published.is_some()
        && watch.attempted.as_ref() == Some(&watch.observed)
        && metadata.as_ref().is_some_and(LocationMetadataSync::paging)
    {
        return None;
    }
    tokio::select! {
        result = stopped => Some(result),
        _ = clock.tick() => None,
    }
}

/// `sqb dev`'s control lines from stdin; outside `sqb dev`, a fixed connected state. `sqb dev` holds stdin open while it lives, so its end, even after `sqb dev` was killed, stops the location as SIGTERM does.
pub(crate) fn dev_controls(options: &LocationServe) -> watch::Receiver<DevControlState> {
    let (sender, receiver) = watch::channel(DevControlState {
        warehouse: (!options.dev_events).then_some(true),
        recompiles: 0,
    });
    if options.dev_events {
        tokio::task::spawn_blocking(move || {
            for line in std::io::stdin().lines().map_while(Result::ok) {
                match parse_dev_control(&line) {
                    Some(DevControl::Warehouse { connected }) => {
                        sender.send_modify(|state| state.warehouse = Some(connected));
                    }
                    Some(DevControl::Recompile) => {
                        sender.send_modify(|state| state.recompiles += 1);
                    }
                    None => {}
                }
            }
            #[cfg(unix)]
            let _ = rustix::process::kill_process(
                rustix::process::getpid(),
                rustix::process::Signal::TERM,
            );
        });
    }
    receiver
}

/// Tell `sqb dev` which inputs changed since the last compile: the first few paths and how many more.
pub(crate) fn report_changed_inputs(before: &[PathStamp], after: &[PathStamp]) {
    let before: BTreeMap<&str, &PathStamp> = before
        .iter()
        .map(|stamp| (stamp.relative_path.as_str(), stamp))
        .collect();
    let after: BTreeMap<&str, &PathStamp> = after
        .iter()
        .map(|stamp| (stamp.relative_path.as_str(), stamp))
        .collect();
    let changed: Vec<&str> = after
        .iter()
        .filter(|(path, stamp)| before.get(*path) != Some(*stamp))
        .map(|(path, _)| *path)
        .chain(
            before
                .keys()
                .copied()
                .filter(|path| !after.contains_key(path)),
        )
        .collect();
    if changed.is_empty() {
        return;
    }
    let shown: usize = changed.len().min(CHANGED_PATHS_SHOWN);
    drop(write_output(&[(
        Stream::Stdout,
        &dev_event_line(&DevEvent::Changed {
            paths: changed[..shown]
                .iter()
                .map(|path| (*path).to_owned())
                .collect(),
            more: changed.len() - shown,
        }),
    )]));
}

/// The location's 250 ms turn; a slow turn skips missed ticks instead of bursting.
pub(crate) fn location_clock() -> Interval {
    let mut clock = tokio::time::interval(Duration::from_millis(250));
    clock.set_missed_tick_behavior(MissedTickBehavior::Skip);
    clock
}

/// The inputs this compile saw; under `sqb dev`, each one changed since the last compile is reported first.
pub(crate) fn note_compiled_inputs(
    options: &LocationServe,
    before: Option<Vec<PathStamp>>,
    observed: &[PathStamp],
) -> Option<Vec<PathStamp>> {
    if !options.dev_events {
        return None;
    }
    if let Some(before) = &before {
        report_changed_inputs(before, observed);
    }
    Some(observed.to_vec())
}

/// Sync a published generation's warehouse state, or defer it until the warehouse connects, then check the location.
pub(crate) async fn settle_publication(
    (options, script, (bearer, session_id)): (&LocationServe, &Path, (&str, Uuid)),
    ((locations, generation), connect, trigger): (
        (Vec<WarehouseStateLocation>, i64),
        bool,
        ConnectionCheckTrigger,
    ),
    (watch, metadata): (LocationWatch, Option<LocationMetadataSync>),
    stopped: impl Future<Output = Result<(), ControlError>>,
) -> Result<Option<Settled>, ControlError> {
    let (mut metadata, deferred) = if connect {
        let mut sync: LocationMetadataSync = metadata_sync(
            (locations, generation),
            (
                options.metadata_reconcile_seconds,
                options.safety_sync_seconds,
            ),
        );
        sync.dynamic_quiet_tokens = metadata
            .map(|previous| previous.dynamic_quiet_tokens)
            .unwrap_or_default();
        (Some(sync), None)
    } else {
        (metadata, Some((locations, generation)))
    };
    let Some((files, wakes_compute)) = check_published_location(
        (options, script, connect),
        ((bearer, session_id), generation, trigger),
        stopped,
    )
    .await?
    else {
        return Ok(None);
    };
    if connect && let Some(sync) = metadata.as_mut() {
        sync.reads_wake_compute = wakes_compute;
    }
    let watch: LocationWatch = exclude_location_databases(&options.project_dir, files, watch);
    Ok(Some((watch, metadata, deferred)))
}

/// Act on what `sqb dev` last asked: a new recompile retries now, and a connected warehouse settles the deferred sync.
pub(crate) async fn apply_dev_controls(
    (options, script, session): (&LocationServe, &Path, (&str, Uuid)),
    (control, recompiles): (DevControlState, u64),
    (mut watch, metadata, deferred): Settled,
    stopped: impl Future<Output = Result<(), ControlError>>,
) -> Result<Option<(u64, Settled)>, ControlError> {
    if control.recompiles != recompiles {
        watch.retry_at = Some(Instant::now());
    }
    let (Some(true), Some((locations, generation))) = (control.warehouse, deferred.clone()) else {
        return Ok(Some((control.recompiles, (watch, metadata, deferred))));
    };
    let settled: Option<Settled> = settle_publication(
        (options, script, session),
        (
            (locations, generation),
            true,
            ConnectionCheckTrigger::Publish,
        ),
        (watch, metadata),
        stopped,
    )
    .await?;
    Ok(settled.map(|settled| (control.recompiles, settled)))
}

/// Open the development store, with its checklist row and the location it names on the screen.
pub(crate) async fn announced_store(
    namespace: &Namespace,
    (project, target): (&Path, &str),
    messages: &UnboundedSender<ScreenMessage>,
) -> Result<(ControlStore, DevelopmentScope), ControlError> {
    drop(messages.send(ScreenMessage::Started {
        step: DevStep::ControlStore,
        detail: String::new(),
    }));
    let database: String = text(namespace, "development_store_url").unwrap_or_else(|| {
        project
            .join(".sqlbuild/control.sqlite3")
            .to_string_lossy()
            .into_owned()
    });
    let (store, scope) = development_store(namespace, (project, target), &database).await?;
    drop(messages.send(ScreenMessage::Finished {
        step: DevStep::ControlStore,
        detail: database,
    }));
    drop(messages.send(ScreenMessage::Location(scope.location_id.to_string())));
    Ok((store, scope))
}

/// The header, size and paths the screen starts with; `plain` asks for appended lines.
pub(crate) fn screen_setup(
    project: &Path,
    (target, adapter): (&str, &str),
    plain: bool,
) -> ScreenSetup {
    let live: bool = live_screen(plain);
    let (columns, rows): (usize, usize) = terminal_size(None).unwrap_or((
        usize::try_from(FALLBACK_COLUMNS).unwrap_or(80),
        FALLBACK_ROWS,
    ));
    ScreenSetup {
        live,
        facts: ScreenFacts {
            columns,
            rows,
            style: CliStyle {
                use_color: screen_colour(live),
            },
            links: live,
        },
        header: DevHeader {
            version: env!("CARGO_PKG_VERSION").to_owned(),
            project: project_name(project),
            target: target.to_owned(),
        },
        adapter: adapter.to_owned(),
        project_root: project.to_string_lossy().into_owned(),
        env_file: env_file_summary(project),
    }
}

/// Complete retained identity and startup pauses before starting any scheduling roles.
pub(crate) async fn development_store(
    namespace: &Namespace,
    (project, target): (&Path, &str),
    database: &str,
) -> Result<(ControlStore, DevelopmentScope), ControlError> {
    set_release_version(env!("CARGO_PKG_VERSION"));
    let store = open_store(database).await?;
    migrate_store(&store).await?;
    let context = AuditContext {
        actor: "local-development".into(),
        granting_role: "control-store-owner".into(),
        request_id: Uuid::now_v7(),
        ip_address: None,
    };
    let location_id = development_location(
        &store,
        (
            &project.to_string_lossy(),
            &development_project_name(project)?,
        ),
        target,
        &context,
    )
    .await?;
    let scope = DevelopmentScope {
        location_id,
        target: target.into(),
        triggers: flag(namespace, "development_triggers"),
        principal_id: development_person(&store, &context).await?,
        allowed_hosts: allowed_hosts(namespace)?,
        listens_beyond_loopback: listens_beyond_loopback(namespace),
    };
    if !scope.triggers {
        pause_development_triggers(&store, (location_id, target)).await?;
    }
    Ok((store, scope))
}

/// After a compile: a publication waits for the `sqb dev` warehouse verdict and settles; a failure keeps the last good snapshot.
pub(crate) async fn finish_compile(
    (options, script, session): (&LocationServe, &Path, (&str, Uuid)),
    (load, generation): (LoadOutcome, i64),
    (mut watch, metadata, deferred): Settled,
    (controls, stopped): (
        &mut watch::Receiver<DevControlState>,
        impl Future<Output = Result<(), ControlError>>,
    ),
) -> Result<Option<Settled>, ControlError> {
    tokio::pin!(stopped);
    let (refresh_at, locations, missing_secrets) = match load {
        Ok(published) => published,
        Err(rules) => {
            watch.retry_at = Some(Instant::now() + Duration::from_secs(30));
            tokio::select! {
                result = &mut stopped => { result?; return Ok(None); },
                () = report_location_health(options, session.0, session.1, LocationHealth::CompileFailed { generation, rules }) => {},
            }
            drop(write_output(&[(
                Stream::Stderr,
                "Location reload failed; retaining last good snapshot. Retrying on change or in 30 seconds.\n",
            )]));
            return Ok(Some((watch, metadata, deferred)));
        }
    };
    watch.retry_at = refresh_at;
    tokio::select! {
        result = &mut stopped => { result?; return Ok(None); },
        () = report_provider_secrets(options, session, (generation, missing_secrets)) => {},
    }
    let trigger = if watch.checked {
        ConnectionCheckTrigger::Publish
    } else {
        ConnectionCheckTrigger::Startup
    };
    watch.checked = true;
    let connect: bool = tokio::select! {
        result = &mut stopped => { result?; return Ok(None); },
        verdict = controls.wait_for(|control| control.warehouse.is_some()) => {
            verdict.is_ok_and(|control| control.warehouse == Some(true))
        },
    };
    settle_publication(
        (options, script, session),
        ((locations, generation), connect, trigger),
        (watch, metadata),
        stopped.as_mut(),
    )
    .await
}

/// Close the metadata reader, take the next generation number and compile it; `None` inside when no number was issued.
pub(crate) async fn compile_next_generation(
    (options, bearer, requests): (&LocationServe, &str, &mpsc::Sender<LocationRuntimeRequest>),
    (metadata, compiled_inputs, observed): (
        Option<LocationMetadataSync>,
        Option<Vec<PathStamp>>,
        &[PathStamp],
    ),
    stopped: impl Future<Output = Result<(), ControlError>>,
) -> Result<Option<NextGeneration>, ControlError> {
    tokio::pin!(stopped);
    let metadata: Option<LocationMetadataSync> = match metadata {
        Some(state) => Some(close_metadata_reader(state).await?),
        None => None,
    };
    let generation: Option<i64> = tokio::select! {
        result = &mut stopped => { result?; return Ok(None); },
        generation = location_generation(options, bearer) => generation?,
    };
    let Some(generation) = generation else {
        return Ok(Some((None, metadata, compiled_inputs)));
    };
    drop(write_output(&[(
        Stream::Stderr,
        &format!("Compiling location generation {generation} in an isolated interpreter...\n"),
    )]));
    let compiled_inputs = note_compiled_inputs(options, compiled_inputs, observed);
    let load: Option<LoadOutcome> = compile_generation(
        (requests, options.dev_events),
        generation,
        observed,
        stopped.as_mut(),
    )
    .await?;
    Ok(load.map(|load| (Some((load, generation)), metadata, compiled_inputs)))
}

/// Ask the runtime interpreter for one plan preview; its reply is one JSON line.
pub(crate) async fn exchange_location_plan(
    compiler: &mut LocationCompiler,
    request: &serde_json::Value,
) -> Result<serde_json::Value, ControlError> {
    let mut line: Vec<u8> = serde_json::to_vec(request).map_err(std::io::Error::other)?;
    line.push(b'\n');
    compiler.stdin.write_all(&line).await?;
    compiler.stdin.flush().await?;
    let mut reply = String::new();
    if compiler.stdout.read_line(&mut reply).await? == 0 {
        return Err(ControlError::Invalid(
            "location interpreter exited before replying".into(),
        ));
    }
    serde_json::from_str(&reply)
        .map_err(|_| ControlError::Invalid("location interpreter returned an invalid plan".into()))
}

/// The safety sync interval from the flag, then the environment, then the project; `None` when unset.
pub(crate) fn safety_sync_setting(
    flag: Option<&str>,
    environment: Option<&str>,
    project: impl FnOnce() -> Result<ControlConfig, String>,
) -> Result<Option<u64>, String> {
    if let Some(text) = flag {
        return sync_interval_seconds(text)
            .map(Some)
            .map_err(|help| format!("--sync-at-least-every {help}"));
    }
    if let Some(text) = environment.filter(|text| !text.trim().is_empty()) {
        return sync_interval_seconds(text)
            .map(Some)
            .map_err(|help| format!("{SAFETY_SYNC_ENV} {help}"));
    }
    Ok(project()?.sync_at_least_every_seconds)
}

/// A plan document that left no diffs out, such as a failure report.
pub(crate) fn without_diffs(
    document: serde_json::Value,
) -> crate::native_commands::types::LocationPlan {
    (document, std::collections::BTreeMap::new())
}

/// The plan document of a plan that failed with its own coded error, its texts redacted and bounded.
pub(crate) fn plan_error(failed: &serde_json::Value) -> serde_json::Value {
    let text = |name: &str| failed[name].as_str().map(redact_error_text);
    json!({"error": {
        "code": text("code").unwrap_or_else(|| "plan_failed".to_owned()),
        "message": text("message").unwrap_or_else(|| PLAN_NOT_PRODUCED.to_owned()),
        "help": text("help"),
    }})
}
