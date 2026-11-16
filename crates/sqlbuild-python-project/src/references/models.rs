//! Public records of references.

use sqlbuild_analysis::explicit_references::types::ResourceKey;

/// One attached-audit ordering edge: the audit, its target, and the gated and read resources.
#[derive(Debug)]
pub struct GateRow {
    pub audit_name: String,
    pub target: ResourceKey,
    pub gated: ResourceKey,
    pub read: ResourceKey,
}

/// One hook-read ordering edge: the hook, the gated and read resources, and its type.
#[derive(Debug)]
pub struct HookRow {
    pub hook_name: String,
    pub gated: ResourceKey,
    pub read: ResourceKey,
    /// The `HookReadType` value.
    pub hook_type: &'static str,
}
