//! The facts cross-file discovery validation reads, in each collection's discovery order.

use crate::models::{DiscoveryFailure, ReadFailure};
use crate::project_validation::errors::CsvDialectError;

/// A resource name and the project-relative path that declares it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct NamedPath {
    pub name: String,
    pub path: String,
}

/// A discovered SQL model file: its stem, path and the declarations its header owns.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ModelFact {
    pub file: NamedPath,
    /// `(name, declaring path)` of the header's enums, then its constants.
    pub local_declarations: Vec<NamedPath>,
}

/// One declarative integration loader a source names.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IntegrationLoaderFact {
    pub kind: String,
}

/// One source entry of a source YAML file.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SourceFact {
    pub name: String,
    pub managed: bool,
    pub loader: Option<String>,
    pub integration_loader: Option<IntegrationLoaderFact>,
}

/// A source YAML file and its entries.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SourceFileFact {
    pub path: String,
    pub entries: Vec<SourceFact>,
}

/// The decorated Python node kinds, in the order discovery lists them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NodeKind {
    Loader,
    Task,
    Asset,
    Check,
}

impl NodeKind {
    /// The kind as resource-name conflicts word it.
    pub fn label(self) -> &'static str {
        match self {
            Self::Loader => "loader",
            Self::Task => "task",
            Self::Asset => "asset",
            Self::Check => "check",
        }
    }
}

/// One dependency a Python node declares.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DependencyFact {
    /// A `SqlResourceRef` to a model, source or seed.
    SqlResource { name: String },
    /// Any other dependency object.
    Object {
        /// The index of the discovered node whose function this object is.
        owner: Option<usize>,
        /// The name a loader, task, asset or check decorator records on the object.
        decorated_name: Option<String>,
    },
}

/// A decorated Python loader, task, asset or check function.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NodeFact {
    pub kind: NodeKind,
    pub name: String,
    pub path: String,
    /// Whether a loader's decorator sets destination, write or schema config.
    pub decorator_config: bool,
    pub depends_on: Vec<DependencyFact>,
}

/// The CSV reader settings a seed declaration authors.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CsvSettings {
    pub delimiter: Option<String>,
    pub quotechar: Option<String>,
    pub escapechar: Option<String>,
    pub doublequote: Option<bool>,
    pub skipinitialspace: Option<bool>,
    /// The error the running Python's `csv.reader` raises for these settings, if any.
    pub dialect_error: Option<CsvDialectError>,
}

/// One seed entry of a schema YAML file.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SeedEntryFact {
    pub name: String,
    pub columns: Vec<String>,
    pub csv: CsvSettings,
}

/// A schema YAML file's model and seed entries.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SchemaFileFact {
    pub path: String,
    pub model_names: Vec<String>,
    pub seeds: Vec<SeedEntryFact>,
}

/// A seed CSV file: its stem and project-relative path, and where to read it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SeedFileFact {
    pub file: NamedPath,
    pub absolute_path: String,
}

/// One SQL test block and its case names.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TestBlockFact {
    pub name: Option<String>,
    pub cases: Vec<String>,
}

/// A SQL test file: its path, stem and blocks.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TestFileFact {
    pub file: NamedPath,
    pub blocks: Vec<TestBlockFact>,
}

/// An audit file: its path and stem, whether it declares a generic audit, and its block names.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AuditFileFact {
    pub file: NamedPath,
    pub generic: bool,
    pub block_names: Vec<Option<String>>,
}

/// An enum or constant declaration file's declarations and whether its scope is private.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ScopedDeclarationsFact {
    pub private: bool,
    pub declarations: Vec<NamedPath>,
}

/// Everything cross-file validation reads about one discovery.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ProjectFacts {
    pub models: Vec<ModelFact>,
    pub sql_functions: Vec<NamedPath>,
    pub python_functions: Vec<NamedPath>,
    pub sql_hooks: Vec<NamedPath>,
    pub seeds: Vec<SeedFileFact>,
    pub schemas: Vec<SchemaFileFact>,
    pub sources: Vec<SourceFileFact>,
    pub tests: Vec<TestFileFact>,
    pub scenarios: Vec<NamedPath>,
    pub audits: Vec<AuditFileFact>,
    /// Enum declaration files, then constant declaration files.
    pub declaration_files: Vec<ScopedDeclarationsFact>,
    /// Every model schema declaration across the model schema files.
    pub model_schemas: Vec<NamedPath>,
    pub materializations: Vec<NamedPath>,
    /// Loaders, tasks, assets, then checks.
    pub nodes: Vec<NodeFact>,
    pub hook_functions: Vec<NamedPath>,
    pub event_exporters: Vec<NamedPath>,
    pub providers: Vec<NamedPath>,
    pub path_default_keys: Vec<String>,
    /// Every model path of the whole project when discovery read only some of its models.
    pub project_model_paths: Option<Vec<String>>,
    /// Python's `csv.field_size_limit()`, or `None` for its default.
    pub csv_field_limit: Option<usize>,
    /// Jobs, job groups, calendars and sensors, read from `orchestration/` without importing it.
    pub orchestration: Vec<OrchestrationNameFact>,
}

/// One name in the project's resource namespace, as a name conflict reports it.
#[derive(Debug)]
pub(crate) struct DeclaredName<'a> {
    pub(crate) name: &'a str,
    pub(crate) kind: &'a str,
    pub(crate) path: &'a str,
    /// A job, job group, calendar or sensor, whose conflicts say how it is named.
    pub(crate) orchestration: bool,
}

/// A named orchestration declaration: its name, its kind as conflicts word it, and its module.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OrchestrationNameFact {
    pub name: String,
    pub kind: &'static str,
    pub path: String,
}

/// Why discovered inputs are rejected: a discovery error, or a seed CSV Python cannot read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ValidationFailure {
    Discovery(DiscoveryFailure),
    Unreadable {
        absolute_path: String,
        failure: ReadFailure,
    },
    /// Python's `csv.Error` reading a seed header, with its message.
    Csv(String),
    /// The settings error the running Python's `csv.reader` raises.
    CsvDialect(CsvDialectError),
}

impl From<DiscoveryFailure> for ValidationFailure {
    fn from(failure: DiscoveryFailure) -> Self {
        Self::Discovery(failure)
    }
}

impl std::fmt::Display for ValidationFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Discovery(failure) => {
                write!(formatter, "{:?}: {}", failure.kind, failure.message)
            }
            Self::Unreadable {
                absolute_path,
                failure,
            } => write!(formatter, "unreadable {absolute_path}: {failure:?}"),
            Self::Csv(message) => write!(formatter, "csv: {message}"),
            Self::CsvDialect(error) => write!(formatter, "{}: {}", error.class, error.message),
        }
    }
}
