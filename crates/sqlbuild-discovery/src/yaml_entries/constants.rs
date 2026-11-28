//! The top-level keys of source and schema YAML documents.

/// The keys a schema YAML file under `seeds/` may declare.
pub(crate) const SEED_FILE_KEYS: [&str; 1] = ["seeds"];
/// The keys a sources YAML file may declare.
pub(crate) const SOURCE_FILE_KEYS: [&str; 2] = ["load", "sources"];
/// The key of a sources file's load.
pub(crate) const LOAD_KEY: &str = "load";
/// What joins a load's name to its table's in the name of a source it fills.
pub(crate) const LOAD_NAME_SEPARATOR: &str = "__";
/// The source entry keys a load decides for the sources it fills, and what each message says to do instead.
pub(crate) const LOAD_OWNED_KEYS: [(&str, &str); 9] = [
    ("database", "the load writes into the target's database"),
    (
        "schema",
        "the load writes into the target's loader_schema, or its own destination_schema",
    ),
    ("table", "the load decides its table"),
    ("expression", "the load decides its table"),
    ("managed", "the load fills it"),
    (
        "write_strategy",
        "set it in the load, with SQLLoad's keys, under `load` or `load.tables`",
    ),
    (
        "cursor_column",
        "set it in the load, with SQLLoad's keys, under `load` or `load.tables`",
    ),
    (
        "unique_key",
        "set it in the load, with SQLLoad's keys, under `load` or `load.tables`",
    ),
    (
        "load_batch_size",
        "set it in the load, with SQLLoad's keys, under `load` or `load.tables`",
    ),
];
/// The load setting naming the schema its tables land in, which the sources it fills read.
pub(crate) const DESTINATION_SCHEMA_KEY: &str = "destination_schema";
/// The loading settings of a source a `@loader` fills, and what to do instead of each.
pub(crate) const LOADER_OWNED_KEYS: [(&str, &str); 4] = [
    (
        "write_strategy",
        "set write_strategy= on @loader, which takes SQLLoad's table options",
    ),
    (
        "cursor_column",
        "set cursor_column= on @loader, which takes SQLLoad's table options",
    ),
    (
        "unique_key",
        "set unique_key= on @loader, which takes SQLLoad's table options",
    ),
    (
        "load_batch_size",
        "remove it; SQLLoad sizes the batches it commits",
    ),
];
