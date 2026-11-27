//! The names a load's SQLLoad project uses, and how SQLLoad paths and patterns are spelled.

/// The project table of SQLLoad's sources, which a load is one of.
pub(crate) const SOURCES: &str = "sources";
/// The project table of SQLLoad's connections.
pub(crate) const CONNECTIONS: &str = "connections";
/// A source's per-table settings.
pub(crate) const TABLES: &str = "tables";
/// A source's tables to sync, as `schema.table` patterns.
pub(crate) const INCLUDE: &str = "include";
/// A table's pinned column types.
pub(crate) const COLUMNS: &str = "columns";
/// How a table's loads meet new and missing columns.
pub(crate) const ON_SCHEMA_CHANGE: &str = "on_schema_change";
/// A source's destination table name template.
pub(crate) const TABLE_NAME: &str = "table_name";
/// A table's own destination name.
pub(crate) const NAME: &str = "name";
/// How a source's tables are named.
pub(crate) const NAMING: &str = "naming";
/// The connection a load reads.
pub(crate) const CONNECTION: &str = "connection";
/// The target a compile check loads into: a session on the project's warehouse that never connects.
pub(crate) const CHECK_TARGET: &str = "__sqlbuild_check";
/// The session connection that target writes through.
pub(crate) const CHECK_CONNECTION: &str = "__sqlbuild_warehouse";
/// The schema it names.
pub(crate) const CHECK_SCHEMA: &str = "__sqlbuild_loads";
/// Where SQLLoad's message names the setting it is about.
pub(crate) const PATH_MARKER: &str = "\nin `";
/// The characters that make an `include` entry a pattern rather than one table.
pub(crate) const GLOB_CHARACTERS: [char; 4] = ['*', '?', '[', '{'];
/// What joins a load's name to its table's in the name of a source it fills.
pub(crate) const LOAD_NAME_SEPARATOR: &str = "__";
/// The key SQLBuild calls a load's settings by in messages.
pub(crate) const LOAD_KEY: &str = "load";
/// How SQLLoad starts a message about a key it does not know.
pub(crate) const UNKNOWN_KEY: &str = "unknown key \"";
/// The `@loader` options SQLBuild decides instead of SQLLoad, and what decides them.
pub(crate) const LOADER_OWNED_KEYS: [(&str, &str); 3] = [
    (
        "columns",
        "declare each column's type on the source it fills, in its sources file",
    ),
    ("table_name", "the source it fills names the table"),
    ("destination_path", "the source it fills names the table"),
];
/// The table option naming how rows are written.
pub(crate) const WRITE_STRATEGY: &str = "write_strategy";
/// SQLBuild's old name for writing the whole table each run, and SQLLoad's.
pub(crate) const RENAMED_STRATEGY: &str = "table";
pub(crate) const REPLACE_STRATEGY: &str = "replace";
/// How SQLLoad pins a warehouse type it passes through verbatim: `native("...")`.
pub(crate) const NATIVE_OPEN: &str = "native(\"";
pub(crate) const NATIVE_CLOSE: &str = "\")";
/// How a `table_name` template marks a placeholder, such as `{table}`.
pub(crate) const TEMPLATE_OPEN: char = '{';
/// The key naming a load's source kind, and the kind of a one-table files source, whose table SQLLoad keys by the source.
pub(crate) const KIND: &str = "kind";
pub(crate) const FILES_KIND: &str = "files";
