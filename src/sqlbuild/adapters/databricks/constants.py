"""Databricks adapter constants."""

NON_ROW_RESULT_COLUMN_NAMES: frozenset[str] = frozenset({"status", "result"})
TABLE_RELATION_METADATA_TYPES: frozenset[str] = frozenset({"managed", "external", "base table"})
DELTA_RELATION_FORMAT: str = "delta"
DELTA_DEFAULT_LOG_RETENTION_DAYS: int = 30
DELTA_DEFAULT_DELETED_FILE_RETENTION_DAYS: int = 7
DELTA_COLUMN_MAPPING_PROPERTY: str = "delta.columnMapping.mode"
DELTA_COLUMN_MAPPING_NAME_MODE: str = "name"
RELATION_NOT_FOUND_ERROR_CLASSES: tuple[str, ...] = (
    "TABLE_OR_VIEW_NOT_FOUND",
    "SCHEMA_NOT_FOUND",
    "CATALOG_NOT_FOUND",
    "NO_SUCH_CATALOG_EXCEPTION",
)
UNDEFINED_TABLE_SQLSTATE: str = "42P01"
DETAILED_TABLE_INFORMATION_MARKER: str = "# detailed table information"
DESCRIBE_DETAIL_ROW_WIDTH: int = 2
TEMPORARY_FUNCTION_PREFIX: str = "__sqb_test__"
MAX_PARAMETER_MARKERS: int = 255
USAGE_NAME_CHUNK_SIZE: int = MAX_PARAMETER_MARKERS - 5
USAGE_UNAVAILABLE_REASON: str = (
    "cannot read system.access.table_lineage (needs Unity Catalog with the access system "
    "schema enabled, and USE CATALOG, USE SCHEMA and SELECT on it)"
)
USAGE_UNAVAILABLE_ERROR_CLASSES: tuple[str, ...] = (
    "INSUFFICIENT_PERMISSIONS",
    "PERMISSION_DENIED",
    "TABLE_OR_VIEW_NOT_FOUND",
    "SCHEMA_NOT_FOUND",
    "CATALOG_NOT_FOUND",
    "NO_SUCH_CATALOG_EXCEPTION",
    "UC_NOT_ENABLED",
)
USAGE_UNAVAILABLE_SQLSTATES: frozenset[str] = frozenset({"42501", "42P01", "42704"})
LINEAGE_VIEW_SOURCE_TYPES: tuple[str, ...] = ("VIEW", "MATERIALIZED_VIEW", "METRIC_VIEW")
CURRENT_CATALOG_INFORMATION_SCHEMA: str = "information_schema"
STRING_LITERAL_ESCAPES: dict[int, str] = str.maketrans({"\\": "\\\\", "'": "\\'"})
