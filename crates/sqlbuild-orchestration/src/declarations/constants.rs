//! Names the declaration reads compare against.

/// The free-form job setting that can carry credentials, so person reads show only that it is set.
pub(crate) const VARS_SETTING: &str = "vars";

/// What a withheld setting shows instead of its value.
pub(crate) const WITHHELD_SETTING: &str = "[REDACTED]";

/// Settings that name another target; a value that isn't a target name, such as a state path, is withheld.
pub(crate) const DEFER_SETTING_PREFIX: &str = "defer_";

/// How many due times a person's trigger listing gives for each running schedule.
pub(crate) const UPCOMING_TIMES: usize = 5;

/// The key prefix of seed nodes, which a job that loads them builds for every reader.
pub(crate) const SEED_KEY_PREFIX: &str = "seed:";

/// Added under a job group's cycles when one of their waits reads a seed or a node several members build.
pub const SHARED_READ_HINT: &str = "hint: a wait in this cycle reads a seed or a node that more than one \
member builds, so every member that reads it waits for all of its builders; give seeds and shared nodes \
their own job that runs first";

/// The published kind of a source node, which group ordering follows to its loader.
pub(crate) const SOURCE_KIND: &str = "source";

/// The published kind of a loader node.
pub(crate) const LOADER_KIND: &str = "loader";

/// Where a published source names the loader that fills it.
pub(crate) const SOURCE_LOADER_POINTER: &str = "/entry/loader";

/// The job setting naming how many models a run builds at once.
pub(crate) const CONCURRENCY_SETTING: &str = "concurrency";

/// A job setting that lets its runs go on past failing audits.
pub(crate) const IGNORE_AUDIT_FAILURES_SETTING: &str = "ignore_audit_failures";
/// The dropped calendar times a republish's health problem names before `and N more`.
pub(crate) const DROPPED_TIMES_NAMED: usize = 10;
