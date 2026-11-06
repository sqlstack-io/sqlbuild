"""Adapter constants."""

INTEGER_TYPE_TOKEN: str = "INT"
TYPED_OBJECT_ENTRY_PART_COUNT: int = 2
TABLE_RELATION_TYPE_NAMES: frozenset[str] = frozenset(
    {"base table", "table", "managed", "external"}
)
VIEW_RELATION_TYPE_NAMES: frozenset[str] = frozenset({"view", "base view"})
DIFF_LEFT_SIDE: str = "left"
DIFF_RIGHT_SIDE: str = "right"
QUALIFIED_NAME_SEPARATOR: str = "."
PYTHON_INIT_FILE_NAME: str = "__init__.py"
PYTHON_IDENTIFIER_REPLACEMENT: str = "_"

SNAPSHOT_VERSION_START_COLUMN: str = "__version_start"
DUCKDB_NOT_FOUND_ERROR_CLASS_NAMES: frozenset[str] = frozenset(
    {"CatalogException", "BinderException"}
)
DUCKDB_MISSING_OBJECT_MARKER: str = "does not exist"
DUCKDB_INTERNAL_ERROR_CLASS_NAME: str = "InternalException"
CANCELLABLE_DURATION_LIMIT_SECONDS: int = 2_147_483
UNSUPPORTED_RELATION_USAGE_REASON: str = "unsupported adapter"

DUCKDB_STORAGE_VERSION_KEY: str = "storage_compatibility_version"
LATEST_DUCKDB_STORAGE_VERSION: str = "latest"
QUOTED_IDENTIFIER_DELIMITER: str = '"'
MIN_QUOTED_IDENTIFIER_LENGTH: int = 3
