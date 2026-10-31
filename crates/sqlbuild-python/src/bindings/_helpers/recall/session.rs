//! The invocation's Recall session: started before the command runs and closed after it returns.

use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime};

use pyo3::exceptions::PyValueError;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use sqlbuild_cli::dispatch::main::closing_status::closing_status;
use sqlbuild_cli::dispatch::main::selector_file_selectors::selector_file_selectors;
use sqlbuild_cli::dispatch::models::CommandFacts;
use sqlbuild_core::command_output::constants::INVOCATION_CONTEXT_ENV;
use sqlbuild_host::entries::models::ExecutorHost;
use sqlbuild_recall::records::main::bounded_selection::bounded_selection;
use sqlbuild_recall::records::main::command_mode::command_mode;
use sqlbuild_recall::records::main::format_timestamp::format_timestamp;
use sqlbuild_recall::records::main::invocation_origin::invocation_origin;
use sqlbuild_recall::records::main::sha256_hex::sha256_hex;
use sqlbuild_recall::records::models::{InvocationContext, OutputStream, RowEnvelope};
use sqlbuild_recall::schema::main::create_database_statement::create_database_statement;
use sqlbuild_recall::schema::main::ddl_script::ddl_script;
use sqlbuild_recall::schema::main::render_script::render_script;
use sqlbuild_recall::schema::models::{DdlOptions, Dialect};
use sqlbuild_recall::settings::main::resolve_recall_settings::resolve_recall_settings;
use sqlbuild_recall::settings::models::{
    AuditEvidence, RecallSettings, SettingsRequest, StoreLocation, StorePurpose,
};
use sqlbuild_recall::store::main::observe_store_file::observe_store_file;
use sqlbuild_recall::store::main::quarantine_store::quarantine_store;
use sqlbuild_recall::store::main::store_location_problem::store_location_problem;
use sqlbuild_recall::store::models::QuarantineOutcome;
use sqlbuild_recall::writer::constants::DEFAULT_HEARTBEAT_INTERVAL_MS;
use sqlbuild_recall::writer::main::start_recall_session::start_recall_session;
use sqlbuild_recall::writer::models::{
    DeliverySummary, RecallSession, SessionConfig, SessionStore, SetupOutcome,
};

use crate::bindings::_helpers::compiled_project::snapshot_execution::WorkerBuild;
use crate::bindings::_helpers::native_host::host;
use crate::bindings::_helpers::output_capture::records::invocation_context;
use crate::bindings::_helpers::recall::clickhouse_store::{
    ClickHouseLocation, ClickHouseStoreOpener, missing_database_reason,
};
use crate::bindings::_helpers::recall::console::write_stderr;
use crate::bindings::_helpers::recall::constants::{
    CLOSE_POLL_MS, DDL_NOTICE_PREFIX, DEBUG_COMMAND, GIT_DIRTY_ENVIRONMENT,
    GIT_REVISION_ENVIRONMENT, HEARTBEAT_ENVIRONMENT, HOST_TOKEN_LENGTH, MINIMUM_HEARTBEAT_SECONDS,
    NO_CONNECTION_ATTRIBUTE, NOTICE_INTERVAL_SECONDS, QUARANTINE_START_WAIT_MS, RECALL_HISTORY,
    STDOUT_STREAM_INDEX,
};
use crate::bindings::_helpers::recall::events::escaped_text;
use crate::bindings::_helpers::recall::replacements::{RecallReplacements, in_replacement_sink};
use crate::bindings::_helpers::recall::sinks::InvocationEvents;
use crate::bindings::_helpers::records::fields::field as native_field;

/// What the invocation records: its session, when enabled, and the resolved target and adapter.
#[derive(Clone, Debug, Default)]
pub(crate) struct InvocationRecall {
    /// The verified worker remains owned until Recall has drained and its claim can be released.
    pub(crate) worker: Arc<Mutex<Option<Arc<WorkerBuild>>>>,
    pub(crate) session: Option<Arc<RecallSession>>,
    pub(crate) target: Option<String>,
    pub(crate) adapter: Option<String>,
    /// The command name, for the closing status line.
    pub(crate) command: String,
    /// The resolved settings and project directory, for notices at the end of the run.
    pub(crate) settings: Option<(RecallSettings, PathBuf)>,
    /// The Recall replacement sinks, configured once runtime extensions are discovered.
    pub(crate) replacements: Arc<RecallReplacements>,
    /// A Recall configuration mistake the command fails with before it runs.
    pub(crate) configuration_error: Option<String>,
    /// The invocation's own lifecycle events the session has recorded.
    pub(crate) invocation_events: Arc<InvocationEvents>,
    /// Which failing audits collect evidence rows, as `recall.audit_evidence` sets it.
    pub(crate) audit_evidence: AuditEvidence,
}

/// The text of an optional attribute value.
fn text_value(value: &Bound<'_, PyAny>) -> PyResult<Option<String>> {
    if value.is_none() {
        return Ok(None);
    }
    escaped_text(&value.str()?).map(Some)
}

fn attribute<'py>(args: &Bound<'py, PyAny>, name: &str) -> PyResult<Bound<'py, PyAny>> {
    args.getattr(name)
}

fn list_attribute(args: &Bound<'_, PyAny>, name: &str) -> PyResult<Vec<String>> {
    let value: Bound<'_, PyAny> = attribute(args, name)?;
    let mut items: Vec<String> = Vec::new();
    if value.is_none() {
        return Ok(items);
    }
    for item in value.try_iter()? {
        items.push(escaped_text(&item?.str()?)?);
    }
    Ok(items)
}

/// The effective selection: positional selectors and `--select`, then what every `--select-file` lists, with the files as passed. A file that cannot be read adds no selectors here; the command reports it.
pub(crate) fn effective_selection(args: &Bound<'_, PyAny>) -> PyResult<(Vec<String>, Vec<String>)> {
    let mut selectors: Vec<String> = list_attribute(args, "select")?;
    let files: Vec<String> = match args.getattr_opt("select_file")? {
        Some(value) if !value.is_none() => value
            .try_iter()?
            .map(|item| escaped_text(&item?.str()?))
            .collect::<PyResult<_>>()?,
        _ => Vec::new(),
    };
    for file in &files {
        if let Some(text) = file_text(Path::new(file)) {
            selectors.extend(selector_file_selectors(&text));
        }
    }
    Ok((selectors, files))
}

fn environment(name: &str) -> Option<String> {
    std::env::var_os(name).map(|value| value.to_string_lossy().into_owned())
}

/// `path`'s text when it exists; Recall metadata is best effort, so unreadable means unknown.
fn file_text(path: &Path) -> Option<String> {
    match std::fs::read_to_string(path) {
        Ok(text) => Some(text),
        Err(error) if error.kind() == ErrorKind::NotFound => None,
        Err(_unreadable) => None,
    }
}

fn project_dir(args: &Bound<'_, PyAny>) -> PyResult<PathBuf> {
    let raw: PathBuf = match text_value(&args.getattr("project_dir")?)? {
        Some(directory) => PathBuf::from(directory),
        None => std::env::current_dir()?,
    };
    Ok(std::fs::canonicalize(&raw).unwrap_or(raw))
}

/// The repository directory a `.git` entry names: the directory itself, or the `gitdir:` a worktree's or submodule's `.git` file points to.
fn git_directory(entry: &Path) -> Option<PathBuf> {
    if entry.is_dir() {
        return Some(entry.to_path_buf());
    }
    let pointer: String = file_text(entry)?;
    let target: &str = pointer.trim().strip_prefix("gitdir:")?.trim();
    let target: PathBuf = PathBuf::from(target);
    let resolved: PathBuf = if target.is_absolute() {
        target
    } else {
        entry.parent()?.join(target)
    };
    resolved.is_dir().then_some(resolved)
}

/// The commit `HEAD` names, read from the repository files without running git; a worktree reads its refs from the common directory.
fn git_revision(project_dir: &Path) -> Option<String> {
    let git: PathBuf = project_dir
        .ancestors()
        .map(|directory| directory.join(".git"))
        .find(|candidate| candidate.exists())
        .and_then(|entry| git_directory(&entry))?;
    let common: PathBuf = file_text(&git.join("commondir"))
        .map_or_else(|| git.clone(), |relative| git.join(relative.trim()));
    let head: String = file_text(&git.join("HEAD"))?;
    let Some(reference) = head.trim().strip_prefix("ref: ") else {
        return Some(head.trim().to_owned());
    };
    for directory in [&git, &common] {
        if let Some(revision) = file_text(&directory.join(reference)) {
            return Some(revision.trim().to_owned());
        }
    }
    for directory in [&git, &common] {
        let Some(packed) = file_text(&directory.join("packed-refs")) else {
            continue;
        };
        let found = packed
            .lines()
            .filter_map(|line| line.split_once(' '))
            .find(|(_, name)| *name == reference);
        if let Some((revision, _)) = found {
            return Some(revision.to_owned());
        }
    }
    None
}

/// The revision a deployment sets in [`GIT_REVISION_ENVIRONMENT`], else the one the repository's files name.
fn recorded_git_revision(project_dir: &Path) -> Option<String> {
    environment(GIT_REVISION_ENVIRONMENT)
        .map(|revision| revision.trim().to_owned())
        .filter(|revision| !revision.is_empty())
        .or_else(|| git_revision(project_dir))
}

/// Whether the working tree had uncommitted changes, as a deployment sets it in [`GIT_DIRTY_ENVIRONMENT`]; unknown otherwise.
fn recorded_git_dirty() -> Option<bool> {
    match environment(GIT_DIRTY_ENVIRONMENT)?
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "1" | "true" | "yes" => Some(true),
        "0" | "false" | "no" => Some(false),
        _ => None,
    }
}

fn host_token() -> Option<String> {
    let name: String = file_text(Path::new("/etc/hostname"))
        .or_else(|| environment("COMPUTERNAME"))
        .or_else(|| environment("HOSTNAME"))?;
    Some(sha256_hex(name.trim().as_bytes())[..HOST_TOKEN_LENGTH].to_owned())
}

/// The orchestrator context as JSON, read from the environment once it is validated; an invalid one records none.
fn external_context(py: Python<'_>) -> PyResult<Option<String>> {
    let context: Bound<'_, PyAny> = match invocation_context(py, None) {
        Ok(context) => context,
        Err(error)
            if error.is_instance(py, &host::entry(py, ExecutorHost::OutputCaptureInputError)?) =>
        {
            return Ok(None);
        }
        Err(error) => return Err(error),
    };
    if !context.is_truthy()? {
        return Ok(None);
    }
    let Some(text) = environment(INVOCATION_CONTEXT_ENV) else {
        return Ok(None);
    };
    serde_json::from_str::<serde_json::Value>(&text)
        .map(|value| Some(value.to_string()))
        .map_err(|error| {
            PyValueError::new_err(format!(
                "{INVOCATION_CONTEXT_ENV} must contain valid JSON: {error}"
            ))
        })
}

fn heartbeat_interval() -> Duration {
    let default = Duration::from_millis(DEFAULT_HEARTBEAT_INTERVAL_MS);
    let Some(value) = environment(HEARTBEAT_ENVIRONMENT) else {
        return default;
    };
    match value.parse::<f64>() {
        Ok(seconds) if seconds.is_finite() && seconds >= MINIMUM_HEARTBEAT_SECONDS => {
            Duration::from_secs_f64(seconds)
        }
        Ok(_) | Err(_) => default,
    }
}

fn invocation_facts(
    py: Python<'_>,
    (args, facts): (&Bound<'_, PyAny>, &CommandFacts),
    (settings, project_dir): (&RecallSettings, &Path),
) -> PyResult<InvocationContext> {
    let fingerprint_source = format!(
        "{}\u{0}{}\u{0}{}",
        settings.project_adapter.as_deref().unwrap_or_default(),
        settings.destination_database.as_deref().unwrap_or_default(),
        settings.destination_schema.as_deref().unwrap_or_default()
    );
    let external_context: Option<String> = external_context(py)?;
    let (selectors, selector_files) = effective_selection(args)?;
    let selection_count: u64 = selectors.len() as u64;
    let (selection, selection_truncated) = bounded_selection(selectors);
    let no_connection: bool = facts.command.as_deref() == Some(DEBUG_COMMAND)
        && attribute(args, NO_CONNECTION_ATTRIBUTE)?.is_truthy()?;
    Ok(InvocationContext {
        origin: Some(invocation_origin(external_context.is_some(), environment).to_owned()),
        command_mode: command_mode(facts.command.as_deref(), no_connection).map(str::to_owned),
        command: facts.command.clone(),
        subcommand: facts.subcommand.clone(),
        selection,
        selection_count: Some(selection_count),
        selection_truncated,
        selector_files,
        exclusion: list_attribute(args, "exclude")?,
        project_name: settings.project_name.clone(),
        project_dir_hash: Some(sha256_hex(project_dir.to_string_lossy().as_bytes())),
        target: settings.target.clone(),
        adapter: settings.project_adapter.clone(),
        destination_database: settings.destination_database.clone(),
        destination_schema: settings.destination_schema.clone(),
        destination_fingerprint: Some(sha256_hex(fingerprint_source.as_bytes())),
        sqlbuild_version: Some(env!("CARGO_PKG_VERSION").to_owned()),
        git_revision: recorded_git_revision(project_dir),
        git_dirty: recorded_git_dirty(),
        host_token: host_token(),
        pid: Some(std::process::id()),
        external_context,
        recall_filter_active: false,
        argv_digest: None,
    })
}

/// Whether a notice for `store` is due, recording it best effort; a missing marker is due.
fn notice_due(project_dir: &Path, store: &str) -> bool {
    let directory: PathBuf = project_dir.join("target");
    let marker: PathBuf = directory.join(format!(
        "{DDL_NOTICE_PREFIX}{}",
        &sha256_hex(store.as_bytes())[..HOST_TOKEN_LENGTH]
    ));
    let modified: std::io::Result<SystemTime> =
        std::fs::metadata(&marker).and_then(|metadata| metadata.modified());
    if let Ok(modified) = modified
        && SystemTime::now()
            .duration_since(modified)
            .is_ok_and(|age| age < Duration::from_secs(NOTICE_INTERVAL_SECONDS))
    {
        return false;
    }
    drop(
        std::fs::create_dir_all(&directory)
            .and_then(|()| std::fs::write(&marker, format_timestamp(SystemTime::now()))),
    );
    true
}

fn target_phrase(settings: &RecallSettings) -> String {
    settings
        .target
        .as_deref()
        .map_or_else(String::new, |target| format!(" for target `{target}`"))
}

/// The DDL notice for an unwritable store, when one is due for it.
fn permission_notice(
    (settings, project_dir): (&RecallSettings, &Path),
    reason: &str,
) -> Option<String> {
    let (dialect, database, place): (Dialect, Option<String>, String) = match &settings.store {
        StoreLocation::Sqlite { path } => (Dialect::Sqlite, None, format!("at {}", path.display())),
        StoreLocation::ClickHouse { database, .. } => (
            Dialect::ClickHouse,
            Some(database.clone()),
            format!("in ClickHouse database `{database}`"),
        ),
        StoreLocation::Unsupported { .. } => return None,
    };
    if !notice_due(project_dir, &place) {
        return None;
    }
    let options = DdlOptions {
        database,
        retention_days: settings.retention_days,
        cluster: None,
    };
    let database_missing: bool = options
        .database
        .as_deref()
        .is_some_and(|database| reason.starts_with(&missing_database_reason(database)));
    let script: String = ddl_script(dialect, &options);
    let script: String = match create_database_statement(&options).filter(|_| database_missing) {
        Some(statement) => format!("{}\n{script}", render_script(&[statement])),
        None => script,
    };
    Some(format!(
        "Recall history is not being written{}: {reason}. An administrator can create the \
         store {place} with:\n\n{script}\n",
        target_phrase(settings),
    ))
}

fn unsupported_notice(
    (settings, project_dir): (&RecallSettings, &Path),
    (connection, adapter): (&str, &str),
) -> Option<String> {
    notice_due(project_dir, connection).then(|| {
        format!(
            "Recall history is not being written{}: connection `{connection}` uses adapter \
             `{adapter}`, which this version cannot store Recall history in.\n",
            target_phrase(settings)
        )
    })
}

/// Write `text` to stderr; a notice that cannot be written never changes the command's outcome.
fn notify(py: Python<'_>, text: Option<String>) {
    if let Some(text) = text {
        drop(write_stderr(py, &text));
    }
}

/// Move a store whose file is not a database aside before the command prints anything.
fn quarantined_notice(path: &Path) -> Option<String> {
    let Ok(Some(observed)) = observe_store_file(path) else {
        return None;
    };
    if !observed.header_foreign {
        return None;
    }
    let give_up: Instant = Instant::now() + Duration::from_millis(QUARANTINE_START_WAIT_MS);
    match quarantine_store(path, &observed, (give_up, SystemTime::now())) {
        Ok(QuarantineOutcome::Moved(moved)) => Some(format!(
            "Recall: {} was not a valid database; it was moved to {} and a fresh store was \
             started.\n",
            path.display(),
            moved.display()
        )),
        Ok(QuarantineOutcome::Kept | QuarantineOutcome::LockBusy) => None,
        Err(error) => Some(format!("Recall history is not being written: {error}.\n")),
    }
}

fn started_session(
    py: Python<'_>,
    (args, identity, facts): (&Bound<'_, PyAny>, &Bound<'_, PyAny>, &CommandFacts),
    (settings, project_dir, hosted): (&RecallSettings, &Path, Option<SessionStore>),
) -> PyResult<Option<Arc<RecallSession>>> {
    let store: SessionStore = match hosted {
        Some(store) => store,
        None => match &settings.store {
            StoreLocation::Sqlite { path } => {
                notify(py, quarantined_notice(path));
                if let Some(reason) = store_location_problem(path) {
                    notify(py, permission_notice((settings, project_dir), &reason));
                    return Ok(None);
                }
                SessionStore::Sqlite(path.clone())
            }
            StoreLocation::ClickHouse {
                config, database, ..
            } => SessionStore::Hosted(Arc::new(ClickHouseStoreOpener {
                location: ClickHouseLocation::new(config, database, settings.retention_days),
                retention_notice: notice_due,
                project_dir: project_dir.to_path_buf(),
                information: Mutex::default(),
            })),
            StoreLocation::Unsupported {
                connection,
                adapter,
            } => {
                notify(
                    py,
                    unsupported_notice((settings, project_dir), (connection, adapter)),
                );
                return Ok(None);
            }
        },
    };
    let Some(invocation_id) = text_value(&native_field(identity, "invocation_id")?)? else {
        return Ok(None);
    };
    let context = invocation_facts(py, (args, facts), (settings, project_dir))?;
    let session = start_recall_session(SessionConfig {
        store,
        maintenance_owner: format!(
            "{}:{}",
            context.host_token.as_deref().unwrap_or("host"),
            std::process::id()
        ),
        envelope: RowEnvelope {
            invocation_id,
            invocation_started_at: format_timestamp(SystemTime::now()),
            producer_version: env!("CARGO_PKG_VERSION").to_owned(),
        },
        context,
        retention_days: settings.retention_days,
        shutdown_timeout: Duration::from_millis(settings.shutdown_timeout_ms),
        diagnostics_min_severity: settings.diagnostics_min_severity.clone(),
        max_output_bytes: settings.max_output_bytes,
        heartbeat_interval: heartbeat_interval(),
        setup_budget: Duration::ZERO,
        maintenance: !worker_invocation(args, facts)?,
        defer_context: true,
        audit_evidence: settings.audit_evidence,
    });
    Ok(Some(Arc::new(session)))
}

fn worker_invocation(args: &Bound<'_, PyAny>, facts: &CommandFacts) -> PyResult<bool> {
    Ok(facts.command.as_deref() == Some("location")
        && attribute(args, "location_command")?
            .extract::<Option<String>>()?
            .as_deref()
            == Some("execute"))
}

fn worker_store(args: &Bound<'_, PyAny>) -> PyResult<SessionStore> {
    let identity = (
        attribute(args, "location_run_id")?
            .extract::<String>()?
            .parse()
            .map_err(|_| PyValueError::new_err("run ID must be a UUID"))?,
        attribute(args, "location_claim_id")?
            .extract::<String>()?
            .parse()
            .map_err(|_| PyValueError::new_err("claim ID must be a UUID"))?,
    );
    let bearer = std::env::var("SQLBUILD_RUN_TOKEN")
        .map_err(|_| PyValueError::new_err("execute requires SQLBUILD_RUN_TOKEN"))?;
    let url = attribute(args, "location_server_url")?.extract::<String>()?;
    args.py().detach(|| {
        sqlbuild_orchestration::worker::main::connect_worker::connect_worker(
            &url, &bearer, identity,
        )
        .map(|client| client.recall_store())
        .map_err(|error| PyValueError::new_err(error.to_string()))
    })
}

/// Begin recording when Recall is enabled, without waiting on the store. Only the write connection is resolved; a write connection without a read connection is kept for the command to fail with, and other unresolvable settings record nothing.
pub(crate) fn start_invocation_recall(
    py: Python<'_>,
    args: &Bound<'_, PyAny>,
    identity: &Bound<'_, PyAny>,
    facts: &CommandFacts,
) -> PyResult<InvocationRecall> {
    let project_dir: PathBuf = project_dir(args)?;
    let request = SettingsRequest {
        project_dir: project_dir.clone(),
        selected_target: text_value(&args.getattr("target")?)?,
        purpose: StorePurpose::Record,
    };
    let mut settings: RecallSettings = match resolve_recall_settings(&request) {
        Ok(settings) => settings,
        Err(error) if error.invalid_configuration => {
            return Ok(InvocationRecall {
                configuration_error: Some(error.message),
                ..InvocationRecall::default()
            });
        }
        Err(_unreachable_store) => return Ok(InvocationRecall::default()),
    };
    let hosted = if worker_invocation(args, facts)? {
        settings.enabled = true;
        Some(worker_store(args)?)
    } else {
        None
    };
    let session: Option<Arc<RecallSession>> = if settings.enabled && settings.project_name.is_some()
    {
        started_session(
            py,
            (args, identity, facts),
            (&settings, &project_dir, hosted),
        )?
    } else {
        None
    };
    let audit_evidence: AuditEvidence = settings.audit_evidence;
    Ok(InvocationRecall {
        worker: Arc::default(),
        session,
        target: settings.target.clone(),
        adapter: settings.project_adapter.clone(),
        command: facts.command.clone().unwrap_or_default(),
        settings: Some((settings, project_dir)),
        replacements: Arc::default(),
        configuration_error: None,
        invocation_events: Arc::default(),
        audit_evidence,
    })
}

/// `notice` as one Recall line, or block when it already ends with a newline.
fn prefixed(notice: &str) -> String {
    if notice.ends_with('\n') {
        format!("Recall: {notice}")
    } else {
        format!("Recall: {notice}.\n")
    }
}

/// What Recall must tell the user about this run once delivery has finished.
fn closing_notices(recall: &InvocationRecall, session: &RecallSession) -> Vec<String> {
    let mut notices: Vec<String> = session
        .notices()
        .iter()
        .map(|notice| prefixed(notice))
        .collect();
    if let (Some(SetupOutcome::PermissionDenied { reason }), Some((settings, project_dir))) =
        (session.wait_for_setup(Duration::ZERO), &recall.settings)
    {
        notices.extend(permission_notice((settings, project_dir), &reason));
        return notices;
    }
    let summary: DeliverySummary = session.summary();
    let total = summary.total();
    if total.dropped > 0 || total.failed > 0 {
        let reason = summary
            .degraded_reason
            .as_deref()
            .map_or_else(String::new, |reason| format!(": {reason}"));
        notices.push(format!(
            "warning: Recall history for this run is incomplete ({} dropped, {} failed){reason}\n",
            total.dropped, total.failed
        ));
    }
    notices
}

/// Close the session within its shutdown budget; a further interrupt stops waiting at once. Notices end with the command's own outcome.
pub(crate) fn finish_invocation_recall(
    py: Python<'_>,
    recall: &InvocationRecall,
    exit_code: Option<i64>,
) -> PyResult<()> {
    let Some(session) = &recall.session else {
        return Ok(());
    };
    session.begin_close();
    while !py.detach(|| session.wait_closed(Duration::from_millis(CLOSE_POLL_MS))) {
        if let Err(interrupt) = py.check_signals() {
            session.abandon();
            return Err(interrupt);
        }
    }
    let information: Vec<String> = session
        .information()
        .iter()
        .map(|text| prefixed(text))
        .collect();
    let notices: Vec<String> = closing_notices(recall, session);
    if !information.is_empty() || !notices.is_empty() {
        let affected: Option<&str> = (!notices.is_empty()).then_some(RECALL_HISTORY);
        notify(
            py,
            Some(
                information.concat()
                    + &notices.concat()
                    + &closing_status(&recall.command, exit_code, affected),
            ),
        );
    }
    Ok(())
}

/// Forward bytes a process stream accepted to `session` when it records `invocation_id`.
pub(crate) fn forward_output(
    session: Option<&RecallSession>,
    invocation_id: &str,
    (stream, data): (usize, &[u8]),
) {
    let Some(session) = session.filter(|session| session.invocation_id() == invocation_id) else {
        return;
    };
    if in_replacement_sink() {
        return;
    }
    let stream = if stream == STDOUT_STREAM_INDEX {
        OutputStream::Stdout
    } else {
        OutputStream::Stderr
    };
    session.append_output(stream, data);
}

/// Forward one diagnostic's JSON to `session` for `invocation_id`; unparsable ones are skipped.
pub(crate) fn forward_diagnostic(
    session: Option<&RecallSession>,
    invocation_id: &str,
    diagnostic_json: &str,
) {
    if let Some(session) = session.filter(|session| session.invocation_id() == invocation_id) {
        drop(session.publish_diagnostic(diagnostic_json));
    }
}
