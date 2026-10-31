//! Resolve native invocations and control-service settings from parsed arguments.

use std::net::{IpAddr, SocketAddr};
use std::path::PathBuf;

use chrono::{DateTime, Utc};
use serde_json::json;
use sqlbuild_orchestration::credentials::models::{CredentialScope, PrincipalKind};
use sqlbuild_orchestration::declarations::models::TriggerChange;
use sqlbuild_orchestration::platform::models::{LocationWork, LocationWorkRequest};
use sqlbuild_orchestration::runs::models::{JobRunRequest, RerunRequest};
use sqlbuild_orchestration::store::errors::ControlError;
use uuid::Uuid;

use crate::argv::constants::{JSON_OPTION, REVIEW_PROG};
use crate::argv::models::{ArgvOutcome, Namespace, Value};
use crate::dispatch::main::invocation_policy::invocation_policy;
use crate::dispatch::models::{CommandFacts, InvocationPolicy};
use crate::native_commands::constants::{
    CLEAN_COMMAND, INIT_COMMAND, RULES_COMMAND, RULES_LIST_ACTION, RULES_SHOW_ACTION,
    SKILLS_COMMAND,
};
use crate::native_commands::errors::PythonNeeded;
use crate::native_commands::models::{
    ControlAdmin, CostOptions, NativeCommand, NativeInvocation, RulesAction, RulesOptions,
    SkillsOptions,
};
use sqlbuild_core::cost_command::constants::DESCENDING_ORDER;

/// Whether `sqb review --json` failed to parse, which Python reports as a JSON document.
pub(crate) fn review_json_failure(arguments: &[String], outcome: &ArgvOutcome) -> bool {
    arguments.iter().any(|argument| argument == JSON_OPTION)
        && outcome
            .usage_failure
            .as_ref()
            .is_some_and(|failure| failure.prog == REVIEW_PROG)
}

pub(crate) fn text(namespace: &Namespace, name: &str) -> Option<String> {
    match namespace.get(name) {
        Some(Value::Str(text) | Value::Path(text)) => Some(text.clone()),
        _ => None,
    }
}

pub(crate) fn flag(namespace: &Namespace, name: &str) -> bool {
    namespace.get(name) == Some(&Value::Bool(true))
}

pub(crate) fn control_admin(namespace: &Namespace) -> Result<ControlAdmin, ControlError> {
    let value = text(namespace, "admin_subject").unwrap_or_default();
    let action = text(namespace, "admin_command");
    if action.as_deref() == Some("create-location") {
        if value.trim().is_empty() {
            return Err(ControlError::Invalid("location name is required".into()));
        }
        return Ok(ControlAdmin::CreateLocation(value));
    }
    if action.as_deref() == Some("create-principal") {
        if value.trim().is_empty() {
            return Err(ControlError::Invalid("principal name is required".into()));
        }
        let kind = match text(namespace, "admin_principal_kind").as_deref() {
            Some("service_account") => PrincipalKind::ServiceAccount,
            _ => PrincipalKind::Person,
        };
        return Ok(ControlAdmin::CreatePrincipal(value, kind));
    }
    let id = Uuid::parse_str(&value)
        .map_err(|_| ControlError::Invalid("a valid subject UUID is required".into()))?;
    match action.as_deref() {
        Some("location-work") => Ok(ControlAdmin::LocationWork(id)),
        Some("check-location") => {
            let request_id =
                Uuid::parse_str(&text(namespace, "admin_event_id").unwrap_or_default())
                    .map_err(|_| ControlError::Invalid("--request-id must be a UUID".into()))?;
            let target = text(namespace, "admin_target").unwrap_or_default();
            let reason = text(namespace, "admin_reason").unwrap_or_default();
            if target.trim().is_empty() || reason.trim().is_empty() {
                return Err(ControlError::Invalid(
                    "target and reason must not be blank".into(),
                ));
            }
            Ok(ControlAdmin::RequestLocationWork(LocationWorkRequest {
                request_id,
                location_id: id,
                target,
                reason,
                work: LocationWork::CheckConnection {},
            }))
        }
        Some("change-trigger" | "cancel-run" | "request-run" | "rerun-run") => {
            let identity_option = if matches!(action.as_deref(), Some("request-run" | "rerun-run"))
            {
                "--run-id"
            } else {
                "--event-id"
            };
            let event_id = Uuid::parse_str(&text(namespace, "admin_event_id").unwrap_or_default())
                .map_err(|_| ControlError::Invalid(format!("{identity_option} must be a UUID")))?;
            let reason = text(namespace, "admin_reason").unwrap_or_default();
            if reason.trim().is_empty() {
                return Err(ControlError::Invalid("--reason must not be blank".into()));
            }
            if matches!(action.as_deref(), Some("request-run" | "rerun-run")) {
                let priority = match namespace.get("admin_priority") {
                    Some(Value::Int(value)) => value.parse::<i64>().map_err(|_| {
                        ControlError::Invalid("--priority must fit a signed 64-bit integer".into())
                    })?,
                    _ => 0,
                };
                if action.as_deref() == Some("rerun-run") {
                    return Ok(ControlAdmin::Rerun(RerunRequest {
                        run_id: event_id,
                        source_run_id: id,
                        from_failure: flag(namespace, "admin_from_failure"),
                        reason,
                        priority,
                    }));
                }
                let target = text(namespace, "admin_target").unwrap_or_default();
                let job_key = text(namespace, "admin_job").unwrap_or_default();
                if target.trim().is_empty() || job_key.trim().is_empty() {
                    return Err(ControlError::Invalid(
                        "target and job must not be blank".into(),
                    ));
                }
                return Ok(ControlAdmin::RequestJobRun(JobRunRequest {
                    run_id: event_id,
                    location_id: id,
                    target,
                    job_key,
                    reason,
                    priority,
                }));
            }
            if action.as_deref() == Some("cancel-run") {
                return Ok(ControlAdmin::CancelRun {
                    event_id,
                    run_id: id,
                    reason,
                });
            }
            let mut change = json!({
                "kind": text(namespace, "admin_trigger_change").unwrap_or_default().replace('-', "_"),
            });
            let cursor = text(namespace, "admin_cursor");
            if let Some(cursor) = &cursor {
                change["time"] = json!(cursor);
            }
            let change: TriggerChange = serde_json::from_value(change)
                .map_err(|error| ControlError::Invalid(format!("trigger change: {error}")))?;
            if cursor.is_some() && !matches!(change, TriggerChange::SetCursor { .. }) {
                return Err(ControlError::Invalid(
                    "--cursor is only valid for set-cursor".into(),
                ));
            }
            Ok(ControlAdmin::ChangeTrigger {
                event_id,
                trigger_id: id,
                change,
                reason,
            })
        }
        Some("issue-location-token" | "issue-principal-token") => {
            let expiry = DateTime::parse_from_rfc3339(
                &text(namespace, "admin_expires_at").unwrap_or_default(),
            )
            .map_err(|_| ControlError::Invalid("--expires-at must be an RFC3339 timestamp".into()))?
            .with_timezone(&Utc);
            if expiry <= Utc::now() {
                return Err(ControlError::Invalid(
                    "token expiry must be in the future".into(),
                ));
            }
            let scope = match action.as_deref() {
                Some("issue-principal-token") => CredentialScope::Principal { principal_id: id },
                _ => CredentialScope::Location { location_id: id },
            };
            Ok(ControlAdmin::IssueToken(scope, expiry))
        }
        Some("revoke-token") => Ok(ControlAdmin::RevokeToken(id)),
        _ => Err(ControlError::Invalid(
            "an administration command is required".into(),
        )),
    }
}

pub(crate) fn control_address(
    namespace: &Namespace,
    standalone: bool,
) -> Result<Option<SocketAddr>, ControlError> {
    if standalone {
        return Ok(None);
    }
    let host: IpAddr = text(namespace, "control_host")
        .unwrap_or_else(|| "127.0.0.1".into())
        .parse()
        .map_err(|_| ControlError::Invalid("--host must be an IP address".into()))?;
    let port = match namespace.get("control_port") {
        Some(Value::Int(value)) => value
            .parse::<u16>()
            .map_err(|_| ControlError::Invalid("--port must be between 0 and 65535".into()))?,
        _ => 8000,
    };
    Ok(Some(SocketAddr::new(host, port)))
}

pub(crate) fn control_database(namespace: &Namespace) -> Result<String, ControlError> {
    let variable = text(namespace, "control_database_env")
        .unwrap_or_else(|| "SQLBUILD_CONTROL_DATABASE_URL".into());
    match std::env::var(&variable) {
        Ok(value) => Ok(value),
        Err(std::env::VarError::NotPresent) => Ok(PathBuf::from(
            text(namespace, "project_dir").unwrap_or_else(|| ".".into()),
        )
        .join(".sqlbuild/control.sqlite3")
        .to_string_lossy()
        .into_owned()),
        Err(std::env::VarError::NotUnicode(_)) => Err(ControlError::Invalid(
            "control database variable must be UTF-8".into(),
        )),
    }
}

/// The strings of a list attribute, or `None` when it holds anything else.
fn texts(namespace: &Namespace, name: &str) -> Option<Vec<String>> {
    match namespace.get(name) {
        None | Some(Value::None) => Some(Vec::new()),
        Some(Value::List(items)) => items
            .iter()
            .map(|item| match item {
                Value::Str(text) => Some(text.clone()),
                _ => None,
            })
            .collect(),
        Some(_) => None,
    }
}

fn rules_command(namespace: &Namespace) -> Result<NativeCommand, PythonNeeded> {
    let action: RulesAction = match text(namespace, "rules_command").as_deref() {
        None | Some(RULES_LIST_ACTION) => RulesAction::List,
        Some(RULES_SHOW_ACTION) => RulesAction::Show,
        Some(_) => return Err(PythonNeeded::Command),
    };
    Ok(NativeCommand::Rules(RulesOptions {
        action,
        selector: text(namespace, "rules_rule_selector"),
        json: flag(namespace, "json"),
    }))
}

fn native_command(
    command: Option<&str>,
    namespace: &Namespace,
) -> Result<NativeCommand, PythonNeeded> {
    match command {
        None | Some(CLEAN_COMMAND) => Ok(NativeCommand::Nothing),
        Some(INIT_COMMAND) => Ok(NativeCommand::Init),
        Some(SKILLS_COMMAND) => Ok(NativeCommand::Skills(SkillsOptions {
            global_install: flag(namespace, "skills_global"),
            targets: texts(namespace, "skills_target").ok_or(PythonNeeded::Arguments)?,
            force: flag(namespace, "skills_force"),
        })),
        Some(RULES_COMMAND) => rules_command(namespace),
        Some("cost") => Ok(NativeCommand::Cost(CostOptions {
            selector: text(namespace, "cost_selector").unwrap_or_else(|| "latest".to_owned()),
            limit: match namespace.get("cost_limit") {
                None | Some(Value::None) => None,
                Some(Value::Int(value)) => {
                    Some(value.parse().map_err(|_| PythonNeeded::Arguments)?)
                }
                _ => return Err(PythonNeeded::Arguments),
            },
            no_limit: flag(namespace, "cost_no_limit"),
            sort: text(namespace, "cost_sort"),
            descending: text(namespace, "cost_order").is_none_or(|order| order == DESCENDING_ORDER),
            since: text(namespace, "cost_since"),
            until: text(namespace, "cost_until"),
            json: flag(namespace, "json"),
            json_path: text(namespace, "json_output").map(PathBuf::from),
            no_color: flag(namespace, "no_color"),
        })),
        Some(_) => Err(PythonNeeded::Command),
    }
}

/// The command `namespace` runs, when it runs without Python; `--debug` always uses Python.
pub(crate) fn native_invocation(namespace: &Namespace) -> Result<NativeInvocation, PythonNeeded> {
    if flag(namespace, "debug") {
        return Err(PythonNeeded::Command);
    }
    let command_name: Option<String> = text(namespace, "command");
    let command: NativeCommand = native_command(command_name.as_deref(), namespace)?;
    let policy: InvocationPolicy = invocation_policy(&CommandFacts {
        command: command_name.clone(),
        ..CommandFacts::default()
    });
    Ok(NativeInvocation {
        command,
        command_name,
        project_dir: text(namespace, "project_dir").map(PathBuf::from),
        target: text(namespace, "target"),
        creates_project: policy.creates_project,
        maintains_skills: policy.maintains_skills,
    })
}
