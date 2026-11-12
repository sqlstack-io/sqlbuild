"""Janitor audit event warehouse storage constants."""

from sqlbuild._native import StateSqlValueType

JANITOR_EVENT_SCHEMA_VERSION: int = 1

COLUMN_EVENT_ID: str = "event_id"
COLUMN_SCHEMA_VERSION: str = "schema_version"
COLUMN_EVENT_TYPE: str = "event_type"
COLUMN_OCCURRED_AT: str = "occurred_at"
COLUMN_RUN_ID: str = "run_id"
COLUMN_RELATION_DATABASE: str = "relation_database"
COLUMN_RELATION_SCHEMA: str = "relation_schema"
COLUMN_RELATION_TYPE: str = "relation_type"
COLUMN_ORIGINAL_NAME: str = "original_name"
COLUMN_ORIGINAL_QUALIFIED_NAME: str = "original_qualified_name"
COLUMN_ARCHIVE_NAME: str = "archive_name"
COLUMN_ARCHIVE_QUALIFIED_NAME: str = "archive_qualified_name"
COLUMN_ARCHIVED_AT: str = "archived_at"

JANITOR_EVENT_COLUMNS: tuple[str, ...] = (
    COLUMN_EVENT_ID,
    COLUMN_SCHEMA_VERSION,
    COLUMN_EVENT_TYPE,
    COLUMN_OCCURRED_AT,
    COLUMN_RUN_ID,
    COLUMN_RELATION_DATABASE,
    COLUMN_RELATION_SCHEMA,
    COLUMN_RELATION_TYPE,
    COLUMN_ORIGINAL_NAME,
    COLUMN_ORIGINAL_QUALIFIED_NAME,
    COLUMN_ARCHIVE_NAME,
    COLUMN_ARCHIVE_QUALIFIED_NAME,
    COLUMN_ARCHIVED_AT,
)

JANITOR_EVENT_REQUIRED_COLUMNS: frozenset[str] = frozenset(
    {
        COLUMN_EVENT_ID,
        COLUMN_SCHEMA_VERSION,
        COLUMN_EVENT_TYPE,
        COLUMN_OCCURRED_AT,
        COLUMN_RUN_ID,
        COLUMN_RELATION_SCHEMA,
        COLUMN_ARCHIVE_NAME,
        COLUMN_ARCHIVE_QUALIFIED_NAME,
        COLUMN_ARCHIVED_AT,
    }
)

JANITOR_EVENT_COLUMN_TYPES: dict[str, StateSqlValueType] = {
    **{column: StateSqlValueType.STRING for column in JANITOR_EVENT_COLUMNS},
    COLUMN_SCHEMA_VERSION: StateSqlValueType.INTEGER,
    COLUMN_OCCURRED_AT: StateSqlValueType.TEXT_TIMESTAMP,
    COLUMN_ARCHIVED_AT: StateSqlValueType.TEXT_TIMESTAMP,
}

JANITOR_EVENT_CHUNK_SIZE: int = 100
