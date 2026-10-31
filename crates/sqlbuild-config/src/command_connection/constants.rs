//! Command connection sources, keys and wording.

pub const CLI_WAREHOUSE_SOURCE: &str = "--warehouse";
pub const CONNECTION_WAREHOUSE_SOURCE: &str = "connection warehouse";
pub const CONNECTION_DEFAULT_WAREHOUSE_SOURCE: &str = "connection default";
pub const CONNECTION_WAREHOUSE_KEY: &str = "warehouse";
pub const CONNECTION_SOURCE_KEY: &str = "source";
pub const DBT_PROFILE_CONNECTION_SOURCE: &str = "dbt_profile";
pub const DUCKDB_ADAPTER: &str = "duckdb";
pub const DUCKDB_MEMORY_DATABASE: &str = ":memory:";
/// Adapters whose connection names a local database file, and the key that names it.
pub const LOCAL_DATABASE_PATH_KEYS: [(&str, &str); 2] =
    [(DUCKDB_ADAPTER, "database"), ("sqlite", "path")];
pub const LOCAL_CONFIG_FILENAME: &str = "sqlbuild_local.toml";
/// Connection keys a dbt profile connection routes on rather than connects with.
pub const DBT_PROFILE_ROUTING_KEYS: [&str; 5] =
    ["source", "profile", "target", "project_dir", "profiles_dir"];
/// Keys that suggest a Snowflake connection was given to the DuckDB adapter.
pub(crate) const SNOWFLAKE_LIKE_KEYS: [&str; 5] =
    ["account", "authenticator", "role", "token", "warehouse"];
pub(crate) const WAREHOUSE_ERROR_CODE: &str = "C261";
pub(crate) const UNSUPPORTED_WAREHOUSE_CODE: &str = "C260";
pub(crate) const SETTING_SNIPPET_INDENT: &str = "            ";
/// Two quotes around at least one character.
pub(crate) const MIN_QUOTED_IDENTIFIER_LENGTH: usize = 3;
/// The warehouse group each CLI command selects a target's default warehouse from.
pub const COMMAND_WAREHOUSE_GROUPS: [(&str, Option<&str>); 32] = [
    ("audit", Some("build")),
    ("build", Some("build")),
    ("check", Some("build")),
    ("clone", Some("build")),
    ("dbt", Some("build")),
    ("load", Some("build")),
    ("plan", Some("build")),
    ("review", Some("build")),
    ("seed", Some("build")),
    ("contract", Some("query")),
    ("cost", Some("query")),
    ("debug", Some("query")),
    ("diff", Some("query")),
    ("freshness", Some("query")),
    ("query", Some("query")),
    ("recall", Some("query")),
    ("janitor", Some("query")),
    ("scenario", Some("query")),
    ("test", Some("query")),
    ("clean", None),
    ("compile", None),
    ("dag", None),
    ("format", None),
    ("init", None),
    ("lineage", None),
    ("location", None),
    ("mv", None),
    ("playground", None),
    ("rename", None),
    ("rules", None),
    ("scope", None),
    ("skills", None),
];
