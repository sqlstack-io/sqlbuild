//! Plain-data refactoring facts and plans as Python's models hold them; offsets count code points.

use serde::{Deserialize, Serialize};

use crate::refactoring::types::PhysicalColumn;

/// One supported refactoring.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RefactorOperation {
    RenameModel,
    MoveModel,
    RenameColumn,
}

/// What one text edit changes, for output grouping.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EditKind {
    Reference,
    Fixture,
    Header,
    Column,
    Migration,
    /// A Python import of a moved module.
    Import,
    /// A job's `select` or `exclude` naming a renamed or moved model.
    Selector,
}

impl EditKind {
    /// The kind's value in output, as Python's `EditKind` enum spells it.
    pub fn value(self) -> &'static str {
        match self {
            Self::Reference => "reference",
            Self::Fixture => "fixture",
            Self::Header => "header",
            Self::Column => "column",
            Self::Migration => "migration",
            Self::Import => "import",
            Self::Selector => "selector",
        }
    }
}

/// What an authored SQL file declares.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SqlFileRole {
    Model,
    Test,
    Scenario,
    Audit,
    Hook,
    Function,
    Schema,
}

/// One requested rename or move.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RefactorRequest {
    pub operation: RefactorOperation,
    pub model_name: String,
    pub new_name: String,
    #[serde(default)]
    pub column_name: Option<String>,
    #[serde(default)]
    pub destination: Option<String>,
    #[serde(default)]
    pub cascade: bool,
}

/// One replacement in an authored file, in offsets of the file before any edit.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextEdit {
    pub start: usize,
    pub end: usize,
    pub replacement: String,
    pub kind: EditKind,
    pub line: usize,
    pub column: usize,
    pub before: String,
    pub after: String,
}

impl TextEdit {
    /// The edit with the display text Python's planner gives it instead of the defaults.
    pub fn with_display(self, before: Option<String>, after: Option<String>) -> Self {
        Self {
            before: before.unwrap_or(self.before),
            after: after.unwrap_or(self.after),
            ..self
        }
    }
}

/// Every edit to one file, and where the file lives afterwards.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileChange {
    pub path: String,
    pub original_path: String,
    pub edits: Vec<TextEdit>,
}

impl FileChange {
    /// Whether the change moves the file.
    pub fn moved(&self) -> bool {
        self.path != self.original_path
    }
}

/// A location a refactoring cannot rewrite safely.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManualLocation {
    pub path: String,
    pub line: Option<usize>,
    pub column: Option<usize>,
    pub reason: String,
}

/// A `migrate_from` declaration the refactoring adds to keep history.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MigrationDeclaration {
    pub model_name: String,
    pub declaration: String,
    pub reason: String,
}

/// Every edit, manual location, and migration of one refactoring.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RefactorPlan {
    pub request: RefactorRequest,
    /// Every resolved move of a batch, in order, the first being `request`; empty otherwise.
    #[serde(default)]
    pub batch: Vec<RefactorRequest>,
    pub changes: Vec<FileChange>,
    pub manual: Vec<ManualLocation>,
    pub blocking: Vec<ManualLocation>,
    pub migrations: Vec<MigrationDeclaration>,
    pub renamed_columns: Vec<(String, String, String)>,
    pub help: Option<String>,
    /// Files the refactoring deliberately leaves in place, and why.
    #[serde(default)]
    pub notes: Vec<ManualLocation>,
}

impl RefactorPlan {
    /// Every request the plan carries out: the batch, or the one request.
    pub fn requests(&self) -> &[RefactorRequest] {
        if self.batch.is_empty() {
            std::slice::from_ref(&self.request)
        } else {
            &self.batch
        }
    }
}

/// One rendered region of an expansion pass, pairing its source range with its output range.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExpansionSpan {
    pub source_start: usize,
    pub source_end: usize,
    pub output_start: usize,
    pub output_end: usize,
}

/// Authored-to-expanded SQL evidence of one model file.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExpansionFacts {
    pub expanded_sql: String,
    pub passes: Vec<Vec<ExpansionSpan>>,
}

/// The relation a model builds.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DestinationFacts {
    pub database: Option<String>,
    pub schema: Option<String>,
    pub name: String,
}

/// Where a column is declared.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocationFacts {
    pub path: String,
    pub line: Option<usize>,
    pub column: Option<usize>,
}

/// One declared column of a model's schema entry.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SchemaColumnFacts {
    pub name: String,
    /// Whether the column declares a `migrate_from`.
    pub migrate_from: bool,
    pub location: Option<LocationFacts>,
}

/// One dependency of a model.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DependencyFacts {
    pub resource_type: String,
    pub name: String,
}

/// Every compiler fact of one model a refactoring reads.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelFacts {
    pub name: String,
    /// The project-relative POSIX path of the model file.
    pub path: String,
    pub deps: Vec<DependencyFacts>,
    pub query_sql: String,
    pub authored_query_sql: String,
    /// The authored `materialized` string config, as `get_config_str` returns it.
    pub materialized: Option<String>,
    /// Whether `migrate_from` is a key of the model config.
    pub declares_migrate_from: bool,
    /// Whether the model config's `migrate_from` value is not `None`.
    pub migrate_from_set: bool,
    pub destination: DestinationFacts,
    pub inferred_columns: Vec<String>,
    /// The columns of the model's schema entry, or `None` without a schema entry.
    pub schema_columns: Option<Vec<SchemaColumnFacts>>,
    /// The expansion of the model's file, if the compiler recorded one.
    pub expansion: Option<ExpansionFacts>,
}

/// The output columns of one source or seed.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourceColumnsFacts {
    pub name: String,
    pub columns: Vec<String>,
}

/// One discovered file and its authored contents.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiscoveredFile {
    pub path: String,
    pub contents: String,
}

/// One discovered non-model SQL file with the SQL bodies discovery found in it, in file order.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthoredFile {
    pub role: SqlFileRole,
    pub path: String,
    pub contents: String,
    pub texts: Vec<String>,
}

/// One compiled unit test, the file that declares it and the models it targets.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnitTestFacts {
    /// The project-relative POSIX path of the test file.
    pub path: String,
    /// The `name` its `TEST` header declares, if any.
    pub name: Option<String>,
    pub target_model_names: Vec<String>,
}

/// Declaration files a model move takes along, and what blocks the move.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeclarationMoves {
    pub moves: Vec<(String, String)>,
    pub blocking: Vec<ManualLocation>,
}

/// One scope-index declaration, keyed by its identity.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlacedDeclaration {
    pub key: String,
    /// `kind:name`.
    pub label: String,
    pub path: String,
    pub line: Option<usize>,
    pub column: Option<usize>,
}

/// A declaration the scope placement moves, and the path it must move to.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelocatedDeclaration {
    pub key: String,
    pub path: String,
    /// The destination role keeps its files in groups, but this file is not in one.
    #[serde(default)]
    pub requires_group: bool,
}

/// The scope placement answer for a model move across folders.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeclarationPlacement {
    /// Relocated declarations in identity order, or `None` when placement is unsettled.
    pub relocated: Option<Vec<RelocatedDeclaration>>,
    /// Why the move preview failed, when it did.
    pub diagnostics: Vec<String>,
    /// Every declaration of the scope index, in index order.
    pub declarations: Vec<PlacedDeclaration>,
}

/// Every compiler fact of one project a refactoring plan reads.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RefactorFacts {
    /// The resolved project directory.
    pub project_dir: String,
    /// The SQL analysis dialect, or `generic`.
    pub dialect: String,
    pub models: Vec<ModelFacts>,
    pub sources: Vec<ResourceColumnsFacts>,
    pub seeds: Vec<ResourceColumnsFacts>,
    /// Discovered model files, in discovery order.
    pub model_files: Vec<DiscoveredFile>,
    /// Discovered test, scenario, audit, hook, function and schema files, in Python's order.
    pub authored_files: Vec<AuthoredFile>,
    /// Discovered source and seed YAML declaration files, in discovery order.
    pub yaml_files: Vec<DiscoveredFile>,
    /// Compiled unit tests, which follow a renamed or moved model they target alone.
    #[serde(default)]
    pub unit_tests: Vec<UnitTestFacts>,
    /// Python strings naming the renamed model or column, found by the Python host.
    pub python_locations: Vec<ManualLocation>,
}

/// One authored model column: name, declared type and the path of its declaring file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContractModelColumn {
    pub name: String,
    pub declared_type: Option<String>,
    pub location_path: Option<String>,
}

/// The model facts `edit_model` reads from a compiled model view.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContractModelRequest {
    pub contents: String,
    pub relative_path: String,
    pub shared_schema: bool,
    pub columns: Vec<ContractModelColumn>,
    /// Each dynamic family's optional `name_pattern`.
    pub dynamic_patterns: Vec<Option<String>>,
    pub physical_columns: Vec<PhysicalColumn>,
    pub overwrite: bool,
}

/// The source facts `edit_source` reads from a compiled source view.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContractSourceRequest {
    /// The text to edit: earlier edits of the same file, or the file as discovered.
    pub contents: String,
    /// The file as discovered, whose first lines may mark it as generated.
    pub discovered_contents: String,
    pub name: String,
    /// Declared `(name, type)` columns.
    pub columns: Vec<(String, Option<String>)>,
    pub physical_columns: Vec<PhysicalColumn>,
    pub overwrite: bool,
}

/// The edited text, or the ownership conflict that prevents a safe edit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ContractEdit {
    Edited(String),
    Conflict(String),
}
