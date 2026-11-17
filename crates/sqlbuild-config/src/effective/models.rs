//! Configuration layers, with authored values kept as opaque handles of type `V`.

use crate::effective::types::Entries;

/// How a target field combines the project layer with the local layer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MergeRule {
    /// `{**project, **local}`.
    Union,
    /// The local value when any `decided_by` local field is set, else the project value.
    Overlay { decided_by: &'static [&'static str] },
    /// Like `Overlay`, but an unset local mapping becomes empty instead of falling back.
    ReplaceOrEmpty { decided_by: &'static [&'static str] },
}

impl MergeRule {
    /// Whether the field holds a mapping rather than a scalar.
    pub fn is_map(self) -> bool {
        !matches!(self, Self::Overlay { .. })
    }
}

/// One `TargetConfig` field: its nested dataclass, if any, its name and its merge rule.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TargetField {
    pub group: Option<&'static str>,
    pub name: &'static str,
    pub rule: MergeRule,
}

/// One target field's authored value; `None` is a local layer leaving it unset.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FieldValue<K, V> {
    Scalar(Option<V>),
    Map(Option<Entries<K, V>>),
}

impl<K, V> FieldValue<K, V> {
    /// Whether the layer sets this field.
    pub fn is_set(&self) -> bool {
        match self {
            Self::Scalar(value) => value.is_some(),
            Self::Map(value) => value.is_some(),
        }
    }
}

/// One layer of a target: each field of `TARGET_FIELDS`, in order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TargetLayer<K, V> {
    pub fields: Vec<FieldValue<K, V>>,
}

/// Every connection layer, lowest precedence first.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConnectionLayers<K, V> {
    pub project: Entries<K, V>,
    /// The selected target's connection name and connection, when a target is selected.
    pub target: Option<(Option<K>, Entries<K, V>)>,
    pub project_named: Entries<K, Entries<K, V>>,
    pub local_named: Entries<K, Entries<K, V>>,
    pub local: Entries<K, V>,
}

/// Where a target's connection comes from, for the only-named-connection default.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConnectionChoice<K> {
    pub target: K,
    /// Whether the target, project or local config already selects or authors a connection.
    pub selected: bool,
    pub project_names: Vec<K>,
    pub local_names: Vec<K>,
}

/// The names that choose a target, highest precedence first, and the targets defined.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TargetChoice<K> {
    pub selected: Option<K>,
    pub local: Option<K>,
    pub default: Option<K>,
    pub defined: Vec<K>,
}

/// The concurrency each configuration layer sets, highest precedence first.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ConcurrencyLayers<'a> {
    /// `--concurrency` on the command line.
    pub requested: Option<i64>,
    /// The selected target's name and its `concurrency`, when it sets one.
    pub target: Option<(&'a str, i64)>,
    /// `[settings] concurrency` after any `sqlbuild_local.toml` override.
    pub settings: i64,
    /// Whether `sqlbuild_local.toml` `[settings]` overrides `concurrency`.
    pub local_override: bool,
}

/// Whether audit failures are ignored at each configuration layer, highest precedence first.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AuditFailureLayers {
    /// The command's flag; a job run's `[settings]` arrive as the flag.
    pub requested: Option<bool>,
    /// The selected target's `ignore_audit_failures`, when it sets one.
    pub target: Option<bool>,
    /// `[settings] ignore_audit_failures` after any `sqlbuild_local.toml` override.
    pub settings: bool,
}

/// Where the effective concurrency came from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConcurrencySource {
    CommandLine,
    Target(String),
    LocalSettings,
    ProjectSettings,
}

impl std::fmt::Display for ConcurrencySource {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CommandLine => formatter.write_str("--concurrency"),
            Self::Target(name) => write!(formatter, "targets.{name}.concurrency"),
            Self::LocalSettings => formatter.write_str("sqlbuild_local.toml settings.concurrency"),
            Self::ProjectSettings => formatter.write_str("settings.concurrency"),
        }
    }
}

/// One command's worker limit and the layer that set it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EffectiveConcurrency {
    pub limit: i64,
    pub source: ConcurrencySource,
}
