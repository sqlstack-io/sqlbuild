//! A load as SQLBuild declares it, and why SQLLoad refuses one.

/// One sources file's load: its settings, and the sources it fills.
#[derive(Clone, Debug, PartialEq)]
pub struct LoadDeclaration {
    pub name: String,
    /// The `load:` mapping without `name`, as SQLLoad reads a `[sources.<name>]` table.
    pub settings: toml::Table,
    /// The sources the file lists that the load fills.
    pub sources: Vec<FilledSource>,
}

/// A listed source a load fills: its name and its declared column types.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FilledSource {
    pub name: String,
    /// Each column with a declared `type`.
    pub columns: Vec<(String, String)>,
    /// How the load meets new and missing columns, as the source declares it.
    pub on_schema_change: Option<String>,
}

/// A declared column type SQLLoad creates as another type.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TypeMismatch {
    pub column: String,
    pub declared: String,
    /// The type SQLLoad creates for `declared`, as the warehouse spells it.
    pub created: String,
    /// What to declare instead: `created` when declaring it creates it exactly, else `created` pinned as `native("...")`.
    pub declare: String,
}

/// What a project's loads are checked against: its source connections and the warehouse kind its targets hold.
#[derive(Clone, Debug)]
pub struct LoadCheck {
    pub connections: toml::Table,
    /// The project's SQLBuild connections, which no load can read: each name, and whether it sets `kind`.
    pub warehouse_connections: Vec<(String, bool)>,
    pub warehouse: sqlload::column_types::models::Warehouse,
    pub motherduck: bool,
}

/// Why a load can't be used: the message, and where in the `load:` mapping it points.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LoadProblem {
    pub message: String,
    /// The keys from the `load:` mapping down to the setting, when the message names one.
    pub path: Vec<String>,
}

/// Where a load runs into: SQLBuild's target, the session it writes through, and its `loader_schema`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LoadTarget {
    pub name: String,
    pub session: String,
    /// The target's `loader_schema`; without one SQLLoad writes to the load's `destination_schema`.
    pub loader_schema: Option<String>,
}

/// A `@loader` as SQLLoad checks it: one Python table under the loader's name, filling one source.
#[derive(Clone, Debug, PartialEq)]
pub struct LoaderDeclaration {
    pub name: String,
    /// The file that defines it, relative to the project.
    pub module: String,
    /// The source connection it reads through `ctx.http` and the rest, if it names one.
    pub connection: Option<String>,
    /// Its SQLLoad table options as written on `@loader`.
    pub settings: toml::Table,
    /// The declared column types of what it fills.
    pub columns: Vec<(String, String)>,
    /// The `on_schema_change` the source it fills declares; `None` when it fills none or that source sets none.
    pub on_schema_change: Option<String>,
    /// Whether it fills a source, which then decides its columns and `on_schema_change`.
    pub fills_source: bool,
    /// The table it fills, which SQLBuild names.
    pub table: String,
}
