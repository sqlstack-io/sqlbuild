"""ClickHouse adapter constants."""

import re

DEFAULT_DATABASE: str = "default"
DEFAULT_HTTP_PORT: int = 8123
DEFAULT_HTTPS_PORT: int = 8443
MAX_QUERY_SIZE_BYTES: int = 262_144
MINIMUM_SERVER_VERSION: tuple[int, ...] = (25, 8)
MAX_IDENTIFIER_LENGTH: int = 200
SESSION_SETTINGS: dict[str, object] = {
    "join_use_nulls": 1,
    "cast_keep_nullable": 1,
    "default_table_engine": "MergeTree",
    "union_default_mode": "DISTINCT",
    "except_default_mode": "DISTINCT",
    "intersect_default_mode": "DISTINCT",
    "mutations_sync": 2,
    "lightweight_deletes_sync": 2,
    "date_time_input_format": "best_effort",
    "cast_string_to_date_time_mode": "best_effort",
    "insert_null_as_default": 0,
}
SETTING_CONFLICT_CODE: str = "A710"
READONLY_SETTING: str = "readonly"
SETTINGS_LOCKED_READONLY: str = "1"
CONNECTION_OPTION_KEYS: frozenset[str] = frozenset(
    {
        "host",
        "port",
        "user",
        "username",
        "password",
        "database",
        "secure",
        "verify",
        "settings",
        "connect_timeout",
        "send_receive_timeout",
        "dsn",
    }
)
VIEW_ENGINES: frozenset[str] = frozenset({"View", "MaterializedView", "LiveView", "WindowView"})
RELATION_NOT_FOUND_CODES: frozenset[int] = frozenset({60, 81})
NULLABLE_WRAPPER: str = "Nullable"
TYPE_WRAPPERS: tuple[str, ...] = ("Nullable", "LowCardinality")
TYPE_ALIASES: dict[str, str] = {
    "Int8": "TINYINT",
    "Int16": "SMALLINT",
    "Int32": "INTEGER",
    "Int64": "BIGINT",
    "Int128": "HUGEINT",
    "Int256": "HUGEINT",
    "UInt8": "SMALLINT",
    "UInt16": "INTEGER",
    "UInt32": "BIGINT",
    "UInt64": "UBIGINT",
    "UInt128": "HUGEINT",
    "UInt256": "HUGEINT",
    "Float32": "REAL",
    "Float64": "DOUBLE",
    "BFloat16": "REAL",
    "String": "VARCHAR",
    "FixedString": "VARCHAR",
    "UUID": "UUID",
    "Bool": "BOOLEAN",
    "Date": "DATE",
    "Date32": "DATE",
    "DateTime": "TIMESTAMP",
    "DateTime64": "TIMESTAMP",
    "Decimal": "DECIMAL",
    "Decimal32": "DECIMAL",
    "Decimal64": "DECIMAL",
    "Decimal128": "DECIMAL",
    "Decimal256": "DECIMAL",
    "JSON": "JSON",
}
REWRITE_STAGING_INFIX: str = "__sqb_rw_"
REWRITE_DELTA_INFIX: str = "__sqb_rd_"
SNAPSHOT_WORK_INFIX: str = "__sqb_snap_work"
SNAPSHOT_REBUILD_INFIX: str = "__sqb_snap_rebuild"
LOW_CARDINALITY_WRAPPER: str = "LowCardinality"
DECIMAL_ALIAS: str = "DECIMAL"
DECIMAL_BASE_TYPE: str = "Decimal"
DECIMAL_PRECISION_BY_BASE: dict[str, int] = {
    "Decimal32": 9,
    "Decimal64": 18,
    "Decimal128": 38,
    "Decimal256": 76,
}
UNWRAPPABLE_TYPE_BASES: frozenset[str] = frozenset(
    {"Array", "Map", "Tuple", "Nested", "JSON", "Object", "Variant", "Dynamic"}
)
VERIFY_OPTION: str = "verify"
DSN_OPTION: str = "dsn"
TIMEOUT_OPTIONS: tuple[str, ...] = ("connect_timeout", "send_receive_timeout")
DEFAULT_COLUMN_DEFAULT_KIND: str = "DEFAULT"
EXCHANGE_UNSUPPORTED_CODES: frozenset[int] = frozenset({1, 48})
STATE_TABLE_ENGINE_KEYWORD: str = "ENGINE"
RELATION_SEPARATOR: str = "."
STATE_TIMESTAMP_TYPE: str = "DateTime64(6, 'UTC')"
PHYSICAL_STORAGE_CONFIG_KEY: str = "physical_storage"
DEFAULT_TABLE_ENGINE: str = "MergeTree"
EMPTY_SORTING_KEY: str = "tuple()"
GENERATION_COMMENT_PREFIX: str = "sqlbuild generation "
EXPECTED_OUTPUT_COLUMNS_CONFIG_KEY: str = "expected_output_columns"
MODEL_NAME_CONFIG_KEY: str = "model_name"
OUTPUT_COLUMN_NAME_CODE: str = "A711"
QUALIFIER_SEPARATOR: str = "."
NULLABLE_KEY_CODE: str = "A712"
NULLABLE_KEY_SETTING: str = "allow_nullable_key = 1"
KEY_IDENTIFIER_PATTERN: re.Pattern[str] = re.compile(r"`([^`]+)`|([A-Za-z_][A-Za-z0-9_]*)")
NULL_IN_ORDINARY_COLUMN_SERVER_CODE: int = 349
NULL_IN_ORDINARY_COLUMN_CODE: str = "A713"
NULL_TARGET_COLUMN_PATTERN: re.Pattern[str] = re.compile(r"to destination column (\S+?):")
FRESH_GENERATION_CLAUSE: str = " COMMENT ''"
USAGE_NAME_CHUNK_SIZE: int = 1_000
USAGE_UNAVAILABLE_REASON: str = (
    "cannot read system.query_log (needs query logging on and SELECT on system.query_log)"
)
USAGE_LOGGING_OFF_REASON: str = "query logging is off for this connection (log_queries = 0)"
USAGE_UNAVAILABLE_ERROR_CODES: frozenset[int] = frozenset({60, 497})
USAGE_QUERY_KINDS: tuple[str, ...] = ("Select", "Insert")
USAGE_ERROR_CODE_PATTERN: re.Pattern[str] = re.compile(r"Code: (\d+)\.")
LOGGING_SETTING_SQL: str = "SELECT getSetting('log_queries')"
LOGGING_OFF_VALUES: frozenset[str] = frozenset({"0", "False"})
REPLICA_COUNT_SQL: str = (
    "SELECT max(replicas) FROM (SELECT count() AS replicas FROM system.clusters "
    "WHERE (cluster, shard_num) IN "
    "(SELECT cluster, shard_num FROM system.clusters WHERE is_local) "
    "GROUP BY cluster, shard_num)"
)
OTHER_REPLICAS_NOTICE: str = "usage read from this server only; other replicas not checked"

EXACT_ROW_COUNT_ENGINES: frozenset[str] = frozenset(
    {"MergeTree", "ReplicatedMergeTree", "SharedMergeTree"}
)
LOWEST_TIMESTAMP_CURSOR: str = "1900-01-01 00:00:00"
HIGHEST_TIMESTAMP_CURSOR: str = "2299-12-31 23:59:59.999999"
