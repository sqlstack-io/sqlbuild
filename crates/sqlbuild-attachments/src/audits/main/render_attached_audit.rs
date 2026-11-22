//! Attached generic audit rendering: argument merge, SQL, severity and run scope.

use crate::audits::_helpers::rendering::{
    merged_arguments, policies, rendered, unreferenced_relationship_target, unused_argument,
};
use crate::audits::models::{ArgumentValue, AuditAttachment, AuditRendering, RenderedAudit};
use sqlbuild_sqltext::sql_scan::models::LexicalSyntax;

/// Render one attachment in `syntax`, or the error rendering raises.
#[must_use]
pub fn render_attached_audit(
    attachment: &AuditAttachment,
    syntax: Option<&LexicalSyntax>,
) -> AuditRendering {
    let rendering = || -> Result<RenderedAudit, String> {
        let arguments: Vec<(String, ArgumentValue)> = merged_arguments(attachment)?;
        let sql_body: String = rendered(attachment, &attachment.sql_body, &arguments, syntax)?;
        let evidence_sql: Option<String> = attachment
            .evidence_sql
            .as_deref()
            .map(|evidence| rendered(attachment, evidence, &arguments, syntax))
            .transpose()?;
        if let Some(message) =
            unused_argument(attachment).or_else(|| unreferenced_relationship_target(attachment))
        {
            return Err(message);
        }
        Ok(RenderedAudit {
            sql_body,
            evidence_sql,
            policies: policies(attachment),
        })
    };
    match rendering() {
        Ok(rendered) => AuditRendering::Rendered(rendered),
        Err(message) => AuditRendering::Failed(message),
    }
}
