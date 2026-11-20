"""Dynamic pause table columns, timings and error codes."""

from sqlbuild._native import StateSqlValueType

DYNAMIC_PAUSE_SCHEMA_VERSION: int = 1
HEARTBEAT_SECONDS: float = 60.0
CLAIM_POLL_SECONDS: float = 1.0
CLAIM_RUN_MARKER: str = "/claim/"
ACTIVE_STATE: str = "ACTIVE"
SUSPENDED_STATE: str = "SUSPENDED"
HELD_ELSEWHERE_CODE: str = "A718"
IGNORE_GRACE_REFUSED_CODE: str = "A719"

COLUMN_EVENT_ID: str = "event_id"
COLUMN_SCHEMA_VERSION: str = "schema_version"
COLUMN_EVENT_TYPE: str = "event_type"
COLUMN_RUN_ID: str = "run_id"
COLUMN_RELATION_DATABASE: str = "relation_database"
COLUMN_RELATION_SCHEMA: str = "relation_schema"
COLUMN_RELATION_NAME: str = "relation_name"
COLUMN_RECORDED_BY: str = "recorded_by"
COLUMN_RECORDED_AT: str = "recorded_at"

DYNAMIC_PAUSE_COLUMNS: tuple[str, ...] = (
    COLUMN_EVENT_ID,
    COLUMN_SCHEMA_VERSION,
    COLUMN_EVENT_TYPE,
    COLUMN_RUN_ID,
    COLUMN_RELATION_DATABASE,
    COLUMN_RELATION_SCHEMA,
    COLUMN_RELATION_NAME,
    COLUMN_RECORDED_BY,
    COLUMN_RECORDED_AT,
)

DYNAMIC_PAUSE_REQUIRED_COLUMNS: frozenset[str] = frozenset(
    {
        COLUMN_EVENT_ID,
        COLUMN_SCHEMA_VERSION,
        COLUMN_EVENT_TYPE,
        COLUMN_RUN_ID,
        COLUMN_RECORDED_BY,
        COLUMN_RECORDED_AT,
    }
)

DYNAMIC_PAUSE_COLUMN_TYPES: dict[str, StateSqlValueType] = {
    **{column: StateSqlValueType.STRING for column in DYNAMIC_PAUSE_COLUMNS},
    COLUMN_SCHEMA_VERSION: StateSqlValueType.INTEGER,
    COLUMN_RECORDED_AT: StateSqlValueType.TEXT_TIMESTAMP,
}
