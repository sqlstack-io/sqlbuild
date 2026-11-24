//! Where one test or scenario runs, and the refusals that keep it off DuckDB.

use crate::run_on::constants::{AUTO, LOCAL, WAREHOUSE};

/// Where a run sends its tests or scenarios: `auto` picks per item.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RunOn {
    #[default]
    Auto,
    Local,
    Warehouse,
}

impl RunOn {
    /// The setting's value, `auto`, `local` or `warehouse`; `None` for any other text.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            AUTO => Some(Self::Auto),
            LOCAL => Some(Self::Local),
            WAREHOUSE => Some(Self::Warehouse),
            _ => None,
        }
    }

    /// The setting's value as configuration spells it.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Auto => AUTO,
            Self::Local => LOCAL,
            Self::Warehouse => WAREHOUSE,
        }
    }
}

/// Every place a run's `RunOn` can be set, from highest precedence to lowest.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RunOnLayers {
    pub flag: Option<String>,
    pub target: Option<String>,
    /// `[settings]`, the local file over the project file.
    pub settings: Option<String>,
}

/// One SQLQuack refusal, or one of SQLBuild's own reasons, placed in the file holding it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LocalRefusal {
    pub category: String,
    pub construct: String,
    pub reason: String,
    pub code: Option<String>,
    pub message: Option<String>,
    pub label: Option<String>,
    pub notes: Vec<String>,
    pub help: Option<String>,
    pub path: Option<String>,
    pub line: Option<i64>,
    pub column: Option<i64>,
    pub width: i64,
    pub snippet: Option<String>,
    pub expanded_from: Option<String>,
}

impl LocalRefusal {
    /// `<category>: <construct>`, as SQLQuack's census labels a refusal.
    #[must_use]
    pub fn title(&self) -> String {
        format!("{}: {}", self.category, self.construct)
    }
}

/// Where one test or scenario runs, `local` or `warehouse`, with the refusals behind `warehouse`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RunOnDecision {
    pub name: String,
    pub run_on: String,
    pub refusals: Vec<LocalRefusal>,
}

impl RunOnDecision {
    /// The first refusal's title; `None` when nothing keeps the item off DuckDB.
    #[must_use]
    pub fn reason(&self) -> Option<String> {
        self.refusals.first().map(LocalRefusal::title)
    }

    /// Whether the item can run locally.
    #[must_use]
    pub fn local(&self) -> bool {
        self.run_on == LOCAL
    }
}
