//! Begin recording when Recall is enabled, without waiting on the store. Only the write connection is resolved; a write connection without a read connection is kept for the command to fail with, and other unresolvable settings record nothing.

use crate::recall::_helpers::session::project_dir;
use crate::recall::_helpers::session::started_session;
use crate::recall::_helpers::session::text_value;
use crate::recall::_helpers::session::worker_invocation;
use crate::recall::_helpers::session::worker_store;
use crate::recall::models::InvocationRecall;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use sqlbuild_cli::dispatch::models::CommandFacts;
use sqlbuild_recall::settings::main::resolve_recall_settings::resolve_recall_settings;
use sqlbuild_recall::settings::models::{
    AuditEvidence, RecallSettings, SettingsRequest, StorePurpose,
};
use sqlbuild_recall::writer::models::RecallSession;
use std::path::PathBuf;
use std::sync::Arc;

/// Begin recording when Recall is enabled, without waiting on the store. Only the write connection is resolved; a write connection without a read connection is kept for the command to fail with, and other unresolvable settings record nothing.
pub fn start_invocation_recall(
    py: Python<'_>,
    (args, identity, facts): (&Bound<'_, PyAny>, &Bound<'_, PyAny>, &CommandFacts),
    sqlbuild_version: &str,
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
            sqlbuild_version,
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
