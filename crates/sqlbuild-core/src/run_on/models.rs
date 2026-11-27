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
    /// Where the caret goes in `snippet` when it is generated SQL rather than the file's line.
    pub snippet_column: Option<i64>,
    pub expanded_from: Option<String>,
    /// The macro the refused construct came from, as `@py.<name>` or `@sql.<name>`.
    pub macro_name: Option<String>,
    /// Where the macro's own source holds the refused construct; `None` when it can't be found there.
    pub macro_place: Option<MacroPlace>,
}

/// The refused construct in a macro's own source file.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MacroPlace {
    pub path: String,
    pub line: i64,
    pub column: i64,
    pub snippet: String,
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
    /// What a local item relied on from the project, such as `HASH (allowed)`.
    pub relied_on: Vec<String>,
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

/// Everything one refused construct blocks: one block in the report, with each place it is written.
#[derive(Clone, Debug, Default)]
pub struct RefusalGroup {
    /// The first refusal, whose code, message, notes and help the block shows once.
    pub refusal: LocalRefusal,
    /// Whether the refusals have no place in a file, such as a project function's.
    pub unplaced: bool,
    pub origins: Vec<RefusalOrigin>,
    /// Every item the construct blocks, in first-seen order.
    pub items: Vec<String>,
}

impl RefusalOrigin {
    /// The models this place is written in or expanded into.
    fn user_models(&self) -> impl Iterator<Item = &str> {
        self.users.iter().map(|user| user.model.as_str())
    }
}

impl RefusalGroup {
    /// What the header and the census call the construct: `fn:HASH`, or `project function`.
    #[must_use]
    pub fn label(&self) -> String {
        if self.unplaced {
            return self.refusal.category.clone();
        }
        self.refusal.construct.clone()
    }

    /// The distinct models the construct is written in or expanded into.
    #[must_use]
    pub fn models(&self) -> usize {
        let mut models: Vec<&str> = self
            .origins
            .iter()
            .flat_map(RefusalOrigin::user_models)
            .collect();
        models.sort_unstable();
        models.dedup();
        models.len()
    }
}

/// One place a construct is written: a macro, a file position, or an unplaced construct's name.
#[derive(Clone, Debug, Default)]
pub struct RefusalOrigin {
    /// The first refusal at this origin, whose place the frame shows.
    pub refusal: LocalRefusal,
    /// The models that read it, each with the items it blocks there.
    pub users: Vec<OriginUser>,
    pub items: Vec<String>,
}

/// One model an origin is written in or expanded into, and the items it blocks there.
#[derive(Clone, Debug, Default)]
pub struct OriginUser {
    pub model: String,
    /// The line of the model file the construct is at, or the macro is called on.
    pub line: i64,
    pub items: Vec<String>,
}
