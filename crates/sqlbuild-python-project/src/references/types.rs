//! Public type aliases of references.

use crate::references::models::{GateRow, HookRow};

/// The audit-gate and hook-read edges a plan injects, which both execution readers need.
pub type ExecutionGates = (Vec<GateRow>, Vec<HookRow>);
