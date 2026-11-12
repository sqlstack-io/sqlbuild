"""Runtime node result storage constants."""

from sqlbuild._native import StateSqlValueType

NODE_RESULT_MATERIALIZED_TRUE_VALUE: str = "true"

COLUMN_NODE_TYPE: str = "node_type"
COLUMN_NODE_NAME: str = "node_name"
COLUMN_TARGET_DATABASE: str = "target_database"
COLUMN_TARGET_SCHEMA: str = "target_schema"
COLUMN_TARGET_NAME: str = "target_name"
COLUMN_RUN_ID: str = "run_id"
COLUMN_STATUS: str = "status"
COLUMN_PAYLOAD_JSON_B64: str = "payload_json_b64"
COLUMN_METADATA_JSON_B64: str = "metadata_json_b64"
COLUMN_ERROR_MESSAGE: str = "error_message"
COLUMN_MATERIALIZED: str = "materialized"
COLUMN_TIMESTAMP: str = "ts"

NODE_RESULT_COLUMNS: tuple[str, ...] = (
    COLUMN_NODE_TYPE,
    COLUMN_NODE_NAME,
    COLUMN_TARGET_DATABASE,
    COLUMN_TARGET_SCHEMA,
    COLUMN_TARGET_NAME,
    COLUMN_RUN_ID,
    COLUMN_STATUS,
    COLUMN_PAYLOAD_JSON_B64,
    COLUMN_METADATA_JSON_B64,
    COLUMN_ERROR_MESSAGE,
    COLUMN_MATERIALIZED,
    COLUMN_TIMESTAMP,
)

NODE_RESULT_COLUMN_TYPES: dict[str, StateSqlValueType] = {
    **{column: StateSqlValueType.STRING for column in NODE_RESULT_COLUMNS},
    COLUMN_TIMESTAMP: StateSqlValueType.TEXT_TIMESTAMP,
}
