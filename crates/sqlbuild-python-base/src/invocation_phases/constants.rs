//! The phases an invocation reports, and the lifecycle event reporting each.

/// The lifecycle event of one finished phase.
pub const INVOCATION_PHASE_EVENT: &str = "invocation_phase";

/// The project's declarations and Python were read.
pub const DISCOVERED_PHASE: &str = "discovered";
/// The project compiled.
pub const COMPILED_PHASE: &str = "compiled";
/// The worker's compile matched the snapshot its run was published from.
pub const MATCHED_PHASE: &str = "matched";
/// One saved discovered set the run used.
pub const DISCOVERED_SET_PHASE: &str = "discovered_set";
/// The warehouse connections opened.
pub const CONNECTED_PHASE: &str = "connected";
/// The warehouse state the plan needs was read.
pub const INSPECTED_PHASE: &str = "inspected";
/// The plan was generated.
pub const PLANNED_PHASE: &str = "planned";
