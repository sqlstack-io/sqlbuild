"""BigQuery adapter constants."""

BOOLEAN_METADATA_TYPE_NAME: str = "BOOLEAN"
BOOLEAN_WIRE_TYPE_NAME: str = "BOOL"
DATE_TYPE_NAME: str = "DATE"
DATETIME_TYPE_NAME: str = "DATETIME"
DECIMAL_METADATA_TYPE_NAMES: frozenset[str] = frozenset({"NUMERIC", "BIGNUMERIC"})
FLOAT_METADATA_TYPE_NAME: str = "FLOAT"
FLOAT_WIRE_TYPE_NAME: str = "FLOAT64"
INTEGER_METADATA_TYPE_NAME: str = "INTEGER"
INTEGER_TYPE_TOKEN: str = "INT"
INTEGER_WIRE_TYPE_NAME: str = "INT64"
NOT_FOUND_ERROR_CLASS_NAME: str = "NotFound"
SELECT_STATEMENT_TYPE: str = "SELECT"
TABLE_NAME_WILDCARD: str = "*"
TIMESTAMP_TYPE_TOKEN: str = "TIMESTAMP"

CLONE_REFUSAL_ERROR_CODE: int = 400
CLONE_REFUSAL_REASONS: frozenset[str] = frozenset({"invalid", "invalidQuery"})
CLONE_REFUSAL_PHRASES: tuple[str, ...] = (
    "cannot clone",
    "can't clone",
    "clone is not supported",
    "clones are not supported",
    "cloning is not supported",
    "not supported for clone",
    "clones and snapshots",
    "clone or snapshot",
)
MAX_CLONE_REFUSAL_ERROR_CHAIN: int = 8
NATIVE_TABLE_TYPE: str = "TABLE"
PENDING_JOB_STATE: str = "PENDING"
PENDING_JOB_POLL_SECONDS: float = 1.0
USAGE_NAME_CHUNK_SIZE: int = 1_000
FORBIDDEN_STATUS: int = 403
NOT_FOUND_STATUS: int = 404
FORBIDDEN_ERROR_CLASS_NAMES: frozenset[str] = frozenset({"Forbidden", "PermissionDenied"})
NOT_FOUND_ERROR_CLASS_NAMES: frozenset[str] = frozenset({"NotFound"})
USAGE_UNAVAILABLE_ERROR_REASONS: frozenset[str] = frozenset({"accessDenied", "notFound"})
USAGE_UNAVAILABLE_MESSAGES: tuple[str, ...] = (
    "access denied",
    "permission bigquery.jobs.listall",
    "permission denied",
)
_CLONE_IDENTIFIER: str = r"(`[^`]*`|[A-Za-z0-9_-]+)"
_CLONE_NAME: str = _CLONE_IDENTIFIER + r"(\s*\.\s*" + _CLONE_IDENTIFIER + r"){0,2}"
CLONE_STATEMENT_PATTERN: str = (
    r"(?is)^(\s|--[^\n]*\n|#[^\n]*\n|/\*([^*]|\*+[^*/])*\*+/)*"
    r"CREATE\s+(OR\s+REPLACE\s+)?(TEMP(ORARY)?\s+)?TABLE\s+(IF\s+NOT\s+EXISTS\s+)?"
    + _CLONE_NAME
    + r"\s+CLONE\s+"
    + _CLONE_NAME
    + r"([^A-Za-z0-9_`-]|$)"
)

STRING_LITERAL_ESCAPES: dict[int, str] = str.maketrans(
    {"\\": "\\\\", "'": "\\'", "\n": "\\n", "\r": "\\r"}
)
