//! Python's argument merge, SQL rendering and policy resolution for one attachment.

use crate::audits::_helpers::parameters::{
    RenderStop, parameter_matches, render_parameterized_sql,
};
use crate::audits::_helpers::policies::{resolve_run_scope, resolve_severity};
use crate::audits::constants::{
    MODEL_RESOURCE, RELATION_REFERENCE_CALLS, RELATIONSHIPS_AUDIT, RELATIONSHIPS_TARGET_ARGUMENT,
    SEED_RESOURCE, SOURCE_RESOURCE,
};
use crate::audits::models::{ArgumentValue, AuditAttachment, PolicySource};
use sqlbuild_sqltext::sql_scan::models::LexicalSyntax;

/// Severity first, then run scope, as Python resolves them after expanding the SQL.
pub(crate) fn policies(
    attachment: &AuditAttachment,
) -> Result<(&'static str, PolicySource), String> {
    let severity: &'static str = if attachment.has_thresholds {
        if attachment.threshold_error {
            "error"
        } else {
            "warn"
        }
    } else {
        resolve_severity(
            attachment.instance_severity.as_deref(),
            attachment.default_severity.as_deref(),
            &format!(
                "{} audit '{}'",
                attachment.owner_label, attachment.definition_name
            ),
        )?
    };
    let run_scope_source: PolicySource = resolve_run_scope(
        attachment.instance_run_scope.as_deref(),
        attachment.default_run_scope.as_deref(),
    )?;
    Ok((severity, run_scope_source))
}

/// One SQL text rendered, or the missing-argument or unsupported-value error.
pub(crate) fn rendered(
    attachment: &AuditAttachment,
    sql: &str,
    arguments: &[(String, ArgumentValue)],
    syntax: Option<&LexicalSyntax>,
) -> Result<String, String> {
    render_parameterized_sql(sql, arguments, syntax).map_err(|stop| match stop {
        RenderStop::MissingArgument(name) => format!(
            "{} is missing argument '{name}' for generic audit '{}'{}",
            attachment.owner_label,
            attachment.definition_name,
            owner_argument_hint(attachment, &name)
        ),
        RenderStop::UnsupportedValue(name) => format!(
            "{} generic audit '{}' argument '{name}' uses an unsupported value",
            attachment.owner_label, attachment.definition_name
        ),
    })
}

/// Why `@model` is missing on a seed or source: each owner passes its own kind's name argument.
fn owner_argument_hint(attachment: &AuditAttachment, missing: &str) -> String {
    let owner_arguments: [&str; 3] = [MODEL_RESOURCE, SOURCE_RESOURCE, SEED_RESOURCE];
    if !owner_arguments.contains(&missing) {
        return String::new();
    }
    let Some((owner_kind, _)) = attachment
        .implicit_arguments
        .iter()
        .find(|(argument, _)| owner_arguments.contains(&argument.as_str()))
    else {
        return String::new();
    };
    format!(
        "; a {owner_kind} audit receives '{owner_kind}' and 'relation', not '{missing}', so read \
         the attached relation with @relation"
    )
}

/// `merge_audit_arguments`: an explicit argument may repeat an implicit one only as equal text.
pub(crate) fn merged_arguments(
    attachment: &AuditAttachment,
) -> Result<Vec<(String, ArgumentValue)>, String> {
    let mut merged: Vec<(String, ArgumentValue)> = attachment
        .implicit_arguments
        .iter()
        .map(|(name, value)| (name.clone(), ArgumentValue::Text(value.clone())))
        .collect();
    for (name, value) in &attachment.explicit_arguments {
        match merged.iter().find(|(existing, _)| existing == name) {
            Some((_, existing)) if existing != value => {
                return Err(format!(
                    "{} audit '{}' must not override implicit {name} from attached context",
                    attachment.owner_label, attachment.definition_name
                ));
            }
            Some(_) => {}
            None => merged.push((name.clone(), value.clone())),
        }
    }
    Ok(merged)
}

/// The error for the first explicit argument neither the audit SQL nor its evidence SQL reads.
pub(crate) fn unused_argument(attachment: &AuditAttachment) -> Option<String> {
    let mut used: Vec<&str> = parameter_matches(&attachment.sql_body)
        .into_iter()
        .map(|item| item.name)
        .collect();
    if let Some(evidence) = &attachment.evidence_sql {
        used.extend(
            parameter_matches(evidence)
                .into_iter()
                .map(|item| item.name),
        );
    }
    let implicit: Vec<&str> = attachment
        .implicit_arguments
        .iter()
        .map(|(name, _)| name.as_str())
        .collect();
    let (name, _) = attachment
        .explicit_arguments
        .iter()
        .find(|(name, _)| !used.contains(&name.as_str()) && !implicit.contains(&name.as_str()))?;
    let definition: String = attachment
        .definition_path
        .as_deref()
        .map_or_else(|| "its built-in definition".to_owned(), str::to_owned);
    Some(format!(
        "{} audit '{}' passes argument '{name}', which the audit's SQL in {definition} never \
         uses; remove '{name}' from the attachment",
        attachment.owner_label, attachment.definition_name
    ))
}

/// The error for a built-in `relationships` audit whose `to` is not a relation reference call.
pub(crate) fn unreferenced_relationship_target(attachment: &AuditAttachment) -> Option<String> {
    if attachment.definition_path.is_some() || attachment.definition_name != RELATIONSHIPS_AUDIT {
        return None;
    }
    let target: &str =
        attachment
            .explicit_arguments
            .iter()
            .find_map(|(name, value)| match value {
                ArgumentValue::Text(text) if name == RELATIONSHIPS_TARGET_ARGUMENT => {
                    Some(text.as_str())
                }
                _ => None,
            })?;
    if RELATION_REFERENCE_CALLS
        .iter()
        .any(|call| target.contains(call))
    {
        return None;
    }
    let name: &str = target.trim().trim_matches(['"', '\'', '`']);
    Some(format!(
        "{} audit 'relationships' argument 'to' is {target}, not a relation reference; write \
         to __ref(\"{name}\") (or __source or __seed) so the audit reads the built relation and \
         runs after it",
        attachment.owner_label
    ))
}
