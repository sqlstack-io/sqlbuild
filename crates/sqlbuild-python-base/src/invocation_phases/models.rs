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
