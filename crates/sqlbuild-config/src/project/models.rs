//! The loaded project configuration, described as the Python objects it projects to.

use crate::models::ConfigValue;

/// One configuration file a loader reads or names in a message.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConfigFile {
    Project,
    LegacyProject,
    Local,
    LegacyLocal,
}

/// The paths of the project and local configuration files and their legacy YAML names.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConfigPaths {
    pub project: std::path::PathBuf,
    pub legacy_project: std::path::PathBuf,
    pub local: std::path::PathBuf,
    pub legacy_local: std::path::PathBuf,
}

/// A loaded value, as the Python value or dataclass instance it projects to.
#[derive(Clone, Debug, PartialEq)]
pub enum Built {
    /// A plain value, including `None`, booleans, integers and raw authored mappings.
    Value(ConfigValue),
    /// A native project configuration record; omitted fields keep their defaults.
    Object(&'static str, Vec<(&'static str, Built)>),
    /// A member of a named `StrEnum`, by value.
    Enum(&'static str, String),
    /// A `dict` with string keys, in insertion order.
    Dict(Vec<(String, Built)>),
    /// A `tuple` of strings.
    Tuple(Vec<String>),
    /// A `frozenset` of strings.
    FrozenSet(Vec<String>),
    /// `Decimal(str(value))` of an integer or float.
    Decimal(ConfigValue),
}

/// A setting value as help text shows it in TOML.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SettingText {
    Bool(bool),
    /// An integer, as its decimal text.
    Integer(String),
    Text(String),
}

/// Where a setting lives: its file name, TOML section and key.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SettingSite<'a> {
    pub file_name: &'a str,
    pub section: &'a str,
    pub key: &'a str,
}

/// `[control]` of `sqlbuild_project.toml`: the server and location this project syncs with.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ControlConfig {
    /// HTTPS origin of the SQLBuild server, as `sqb login --server` takes it.
    pub server: Option<String>,
    /// Name of the location whose warehouse copy a signed-in `sqb build` refreshes.
    pub location: Option<String>,
    /// Seconds between safety syncs of `sqb location serve`; `Some(0)` turns them off.
    pub sync_at_least_every_seconds: Option<u64>,
}

/// Each declared pool's limit, and its limit on each target in `[targets]`, overrides applied.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PoolLimits {
    pub declared: std::collections::BTreeMap<String, u32>,
    pub targets: std::collections::BTreeMap<String, std::collections::BTreeMap<String, u32>>,
}

impl PoolLimits {
    /// The limits on `target`: its overrides when it is in `[targets]`, otherwise the declared ones.
    #[must_use]
    pub fn on(&self, target: &str) -> &std::collections::BTreeMap<String, u32> {
        self.targets.get(target).unwrap_or(&self.declared)
    }
}

/// `[changes]` of `sqlbuild_project.toml`: what `--changed` counts and how far a change reaches.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ChangesConfig {
    /// Non-empty, each one of `CHANGE_COUNTS`.
    pub counts: Option<Vec<String>>,
    /// One of `CHANGE_REACHES`.
    pub reach: Option<String>,
}

/// `[dynamic]` of `sqlbuild_project.toml`: when a build may take over paused dynamic models.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DynamicConfig {
    /// Seconds a pausing build's heartbeat may be silent before another build resumes its models.
    pub heartbeat_grace_seconds: u64,
    /// Whether `sqb build --ignore-dynamic-grace` may take over within the grace.
    pub allow_ignore_grace: bool,
}

/// `[review]` of `sqlbuild_project.toml`: the defaults `sqb review` uses when flags are omitted.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ReviewConfig {
    pub against: Option<String>,
    pub downstream: Option<String>,
    pub schema_prefix: Option<String>,
    pub critical_tags: Option<Vec<String>>,
}
