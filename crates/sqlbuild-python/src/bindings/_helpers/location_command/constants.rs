use sqlbuild_orchestration::runs::models::RunCommand;

/// Location actions accepted by the interpreter-side one-shot publisher.
pub(crate) const PUBLISH_ACTION: &str = "publish";

/// Workers verify the pinned nodes and environment without reevaluating declarations.
pub(crate) const EXECUTE_ACTION: &str = "execute";

/// The native implementation each frozen run command dispatches to.
pub(crate) const COMMAND_IMPLEMENTATIONS: [(RunCommand, &str); 5] = [
    (RunCommand::Build, "run_build_command"),
    (RunCommand::Test, "run_test_command"),
    (RunCommand::Seed, "run_seed_command"),
    (RunCommand::Audit, "run_audit_command"),
    (RunCommand::Freshness, "run_freshness_command"),
];
