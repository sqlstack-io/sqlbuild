"""Where tests and scenarios run, SQLBuild's own reasons, and the SQLQuack vocabulary they read."""

from __future__ import annotations

RUN_ON_LOCAL: str = "local"
RUN_ON_WAREHOUSE: str = "warehouse"

LOCAL_DIALECT: str = "duckdb"
CONVERTED_DIALECT: str = "snowflake"

DIALECT_REASON_CATEGORY: str = "dialect"
FUNCTION_REASON_CATEGORY: str = "project function"
RELATION_REASON_CATEGORY: str = "warehouse relation"
SNAPSHOT_MISSING_CATEGORY: str = "snapshot missing"
SNAPSHOT_STALE_CATEGORY: str = "snapshot stale"
SNAPSHOT_INVALID_CATEGORY: str = "snapshot invalid"

SQLQUACK_SOURCES_HELP: str = "a source file of the project (`sources/*.yml`)"
SQLBUILD_SOURCES_HELP: str = "a source file of the project (`sql/sources/`)"
SQLQUACK_ALLOW_HELP: str = (
    r"allow it with (?P<key>\w+) = true under \[allow\] in a sqlquack\.toml in the project folder"
)
"""SQLQuack's help for a known difference, which SQLBuild's own setting replaces."""
SQLBUILD_ALLOW_HELP: str = (
    "allow it for local tests in sqlbuild_project.toml with "
    "`[testing.local] allow = {{ {key} = true }}`"
)
"""Where SQLBuild allows a known difference for local tests (CHI-1688), formatted with its key."""

TIMEZONE_PARAMETER: str = "TIMEZONE"
SESSION_PARAMETERS_KEY: str = "session_parameters"
UTC_TIME_ZONE: str = "UTC"

STATEMENT_CONSTRUCT: str = "statement"

CONVERSION_CACHE_DIR: str = "target/sqlquack-cache"
CONVERSION_CACHE_INDEX: str = "used.json"
CONVERSION_CACHE_LIFETIME_SECONDS: float = 7 * 24 * 60 * 60

CANNOT_RUN_LOCALLY_CODE: str = "T007"
CANNOT_RUN_LOCALLY_HELP: str = "Run this test on the warehouse with --run-on warehouse."
GUARD_CATEGORY: str = "SQLQuack guard"
DUCKDB_ERROR_CATEGORY: str = "DuckDB internal error"
