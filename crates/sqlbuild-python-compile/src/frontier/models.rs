//! Public records of frontier.

/// The compiler stages native callers produce through the frontier.
#[derive(Debug, Clone, Copy)]
pub enum FrontierStage {
    DiscoveredProjectInputs,
    CompileProjectInputs,
    CompiledProject,
}
