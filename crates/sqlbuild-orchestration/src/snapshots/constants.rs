//! Snapshot retention keeps recent publications per location and target, collected in small batches.

use crate::discovered_sets::constants::{DISCOVERED_ASSETS_DOCUMENT, DISCOVERED_DOCUMENT};

pub(crate) const DEFAULT_SNAPSHOT_RETENTION_COUNT: u16 = 10;
pub(crate) const SNAPSHOT_RETENTION_BATCH: usize = 10;
pub(crate) const CONTENT_HASH_CHUNK: usize = 500;
/// Nodes a whole-snapshot read decodes per page, each page read in its own short transaction.
pub(crate) const SNAPSHOT_NODE_PAGE: usize = 250;
/// The memory, in MiB, that whole-snapshot reads may decode into at once; a read that would exceed it waits.
pub(crate) const SNAPSHOT_DECODE_BUDGET_MIB: u32 = 512;
/// A definition's decoded size, estimated as this many times its compressed size.
pub(crate) const DECODED_BODY_EXPANSION: u64 = 24;
/// Decoded snapshots kept for every reader to share, the least recently used dropped first.
pub(crate) const SNAPSHOT_NODE_CACHE_ENTRIES: usize = 2;

/// `{node_key: {column: {"expression_range": [start, end]}}}`; readers skip unknown column keys.
pub(crate) const COLUMNS_DOCUMENT: &str = "columns";
/// How long a publication waits for the server, which stores a large snapshot well past 30 s.
pub(crate) const PUBLISH_TIMEOUT_SECONDS: u64 = 300;
/// The target's non-secret connection settings, a `ConnectionDetails`.
pub const CONNECTION_DOCUMENT: &str = "connection";
/// Snapshot document kinds: per-snapshot metadata that never changes a node hash or node change.
pub(crate) const SNAPSHOT_DOCUMENT_KINDS: [&str; 10] = [
    "jobs",
    "groups",
    "triggers",
    "roles",
    "lineage",
    COLUMNS_DOCUMENT,
    "providers",
    CONNECTION_DOCUMENT,
    DISCOVERED_DOCUMENT,
    DISCOVERED_ASSETS_DOCUMENT,
];
/// Connection keys that may name the signed-in user, in preference order.
pub(crate) const USER_KEYS: [&str; 3] = ["user", "username", "login"];
/// Connection keys that may name the server, in preference order.
pub(crate) const HOST_KEYS: [&str; 3] = ["host", "server_hostname", "server"];
/// Connection keys that may name the default database, in preference order.
pub(crate) const DATABASE_KEYS: [&str; 4] = ["database", "dbname", "catalog", "project"];
/// Adapters whose connection names a local database file, and the key that names it.
pub(crate) const FILE_DATABASE_KEYS: [(&str, &str); 2] =
    [("duckdb", "database"), ("sqlite", "path")];
/// Snowflake `authenticator` values by the sign-in method they name; others are shown as they are.
pub(crate) const AUTHENTICATOR_METHODS: [(&str, &str); 7] = [
    ("snowflake", "password"),
    ("snowflake_jwt", "key_pair"),
    ("externalbrowser", "external_browser"),
    ("oauth", "oauth"),
    ("oauth_authorization_code", "oauth"),
    ("oauth_client_credentials", "oauth"),
    ("programmatic_access_token", "token"),
];
/// Keys whose presence names the sign-in method when no authenticator does, in preference order.
pub(crate) const CREDENTIAL_KEY_METHODS: [(&str, &str); 7] = [
    ("private_key", "key_pair"),
    ("private_key_path", "key_pair"),
    ("private_key_file", "key_pair"),
    ("token", "token"),
    ("access_token", "token"),
    ("credentials_path", "service_account"),
    ("password", "password"),
];

/// Beyond the newest generations, any retained reference keeps a snapshot.
pub(crate) const EXPIRED_SNAPSHOTS: &str = "SELECT s.snapshot_id FROM sqb_snapshots s \
    WHERE (SELECT COUNT(*) FROM sqb_snapshots newer WHERE newer.location_id=s.location_id \
        AND newer.target=s.target AND newer.generation>s.generation)>=$1 \
    AND NOT EXISTS (SELECT 1 FROM sqb_current_snapshots c WHERE c.snapshot_id=s.snapshot_id) \
    AND NOT EXISTS (SELECT 1 FROM sqb_runs r WHERE r.snapshot_id=s.snapshot_id) \
    AND NOT EXISTS (SELECT 1 FROM sqb_jobs j WHERE j.snapshot_id=s.snapshot_id) \
    AND NOT EXISTS (SELECT 1 FROM sqb_triggers t WHERE t.snapshot_id=s.snapshot_id \
        OR t.removed_in_snapshot_id=s.snapshot_id) \
    AND NOT EXISTS (SELECT 1 FROM sqb_location_work w WHERE w.snapshot_id=s.snapshot_id) \
    ORDER BY s.published_at, s.snapshot_id LIMIT $2";

/// Content-addressed bodies of the expiring snapshots, removed once nothing else references them.
pub(crate) const CONTENT: [((&str, &str), &str, &str); 3] = [
    (
        ("sqb_snapshot_nodes", "node_hash"),
        "sqb_node_definitions",
        "",
    ),
    (
        ("sqb_snapshot_sources", "text_hash"),
        "sqb_source_texts",
        " AND NOT EXISTS (SELECT 1 FROM sqb_warehouse_fingerprints f \
         WHERE f.definition_text_hash=sqb_source_texts.text_hash)",
    ),
    (
        ("sqb_snapshot_documents", "document_hash"),
        "sqb_documents",
        "",
    ),
];
/// Node kinds a build writes when they are selected.
pub(crate) const WRITER_NODE_KINDS: [&str; 4] = ["model", "seed", "udf", "table_fn"];
pub(crate) const MODEL_NODE_KIND: &str = "model";
pub(crate) const SOURCE_NODE_KIND: &str = "source";
pub(crate) const SOURCE_KEY_PREFIX: &str = "source:";
/// A source with a non-null loader here is managed, so automatic source loading writes it.
pub(crate) const LOADER_POINTER: &str = "/entry/loader";
/// Source writers a loading build of the node plans beyond its `deps`: hook and audit reads, loader chains.
pub(crate) const LOADS_FIELD: &str = "loads";
/// The frozen setting for `--no-load-sources`; `false` stops a build loading sources its models read.
pub(crate) const LOAD_SOURCES_SETTING: &str = "load_sources";
