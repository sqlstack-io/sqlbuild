"""SQL test executor diagnostic codes."""

import re

from sqlbuild.runtime.execution_limits.constants import ITEM_TIMEOUT_CODE

SQL_TEST_TOO_LARGE_CODE: str = "T001"
SQL_TEST_EXECUTION_ERROR_CODE: str = "T002"
SQL_TEST_ASSERTION_FAILED_CODE: str = "T003"
SQL_TEST_ENTRY_MISSING_CODE: str = "T004"
SQL_TEST_EXPECTED_COLUMNS_CODE: str = "T005"
SQL_TEST_SIZE_BYTES: str = "bytes"
SQL_TEST_TIMEOUT_CODE: str = ITEM_TIMEOUT_CODE
INLINED_MODEL_DISPLAY_LIMIT: int = 8
MINUTE_SUGGESTION_THRESHOLD_SECONDS: int = 120
SECONDS_PER_MINUTE: int = 60

SQLQUACK_EXECUTABLE_ENV: str = "SQLBUILD_SQLQUACK"
SQLQUACK_EXECUTABLE_NAME: str = "sqlquack"
LOCAL_TEST_INSTALL_HELP: str = (
    f"Put the `{SQLQUACK_EXECUTABLE_NAME}` executable on PATH, or set "
    f"{SQLQUACK_EXECUTABLE_ENV} to its path."
)
SQLQUACK_POLICY_FILE: str = "sqlquack.toml"
SQLQUACK_MINIMUM_VERSION: tuple[int, int, int] = (0, 2, 0)
SQLQUACK_VERSION_PATTERN: re.Pattern[str] = re.compile(r"^sqlquack (\d+)\.(\d+)\.(\d+)$")
SQLQUACK_VERSION_TIMEOUT_SECONDS: int = 30
SQLQUACK_CONVERT_TIMEOUT_SECONDS: int = 300
SQLQUACK_FUNCTION_REFUSED_PREFIX: str = "sqlquack function: refused: "
SQLQUACK_FUNCTION_FAILED_PREFIX: str = "sqlquack function: failed: "
PLAIN_FUNCTION_NAME: re.Pattern[str] = re.compile(r"^[A-Za-z_][A-Za-z0-9_]*$")
SESSION_NAME_SEPARATOR: str = "="
LOCAL_SAMPLE_TIMEOUT_SECONDS: int = 30
LOCAL_PLAN_ACTION: str = "local"
LOCAL_COMPARISON_COLUMNS: int = 6
CONNECTION_SESSION_KEYS: dict[str, str] = {"timezone": "TIMEZONE"}
SESSION_PARAMETERS_KEY: str = "session_parameters"
SOURCE_DIALECT: str = "snowflake"
LOCAL_DUCKDB_CONNECTION: dict[str, object] = {
    "database": ":memory:",
    "settings": {
        "TimeZone": "UTC",
        "autoinstall_known_extensions": "false",
        "enable_external_access": "false",
    },
}
GUARD_RAISE_PATTERN: re.Pattern[str] = re.compile(r"'([a-z][a-z0-9_]*)#[0-9a-f?]+: ([^']*)'")
REFUSAL_DISPLAY_LIMIT: int = 5
LOCAL_TEST_REFUSED_CODE: str = "T009"
LOCAL_TEST_ENGINE_CODE: str = "T007"
