//! `effective_ignore_audit_failures`: whether a build lets failing audits hold nothing back.

use crate::effective::models::AuditFailureLayers;

/// The command's flag (a job run's settings arrive as it), else the target, else `[settings]`.
#[must_use]
pub fn effective_ignore_audit_failures(layers: &AuditFailureLayers) -> bool {
    layers
        .requested
        .or(layers.target)
        .unwrap_or(layers.settings)
}
