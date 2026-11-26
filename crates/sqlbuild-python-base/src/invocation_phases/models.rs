//! One finished invocation phase.

/// What a finished phase reports: its name, how long it took, a count, a subject and sub-steps.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct InvocationPhase<'a> {
    /// Such as `discovered`, `compiled`, `matched`, `discovered_set`, `connected`, `inspected` or `planned`.
    pub phase: &'a str,
    pub duration_ms: Option<f64>,
    /// What the phase counted: models compiled, nodes selected or a set's items.
    pub count: Option<i64>,
    /// What the phase is about: an adapter, a snapshot version or a discovered set.
    pub name: Option<String>,
    /// The phase's sub-steps or facts, joined with ` · `.
    pub detail: Option<String>,
}

/// A finished phase kept back until its summary is known: how long it took and its facts.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct HeldPhase {
    pub duration_ms: Option<f64>,
    pub detail: Option<String>,
}

impl From<InvocationPhase<'_>> for HeldPhase {
    fn from(phase: InvocationPhase<'_>) -> Self {
        Self {
            duration_ms: phase.duration_ms,
            detail: phase.detail,
        }
    }
}
