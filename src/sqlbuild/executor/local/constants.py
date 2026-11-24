"""Where tests and scenarios run, and the SQLQuack vocabulary local runs read."""

from __future__ import annotations

RUN_ON_LOCAL: str = "local"
RUN_ON_WAREHOUSE: str = "warehouse"
RUN_ON_AUTO: str = "auto"

# The dialect whose SQL runs locally as authored, and the one SQLQuack converts to DuckDB.
LOCAL_DIALECT: str = "duckdb"
CONVERTED_DIALECT: str = "snowflake"

# SQLBuild's own reasons, in SQLQuack's `<category>: <construct>` label form.
DIALECT_REASON_CATEGORY: str = "dialect"
FUNCTION_REASON_CATEGORY: str = "project function"
RELATION_REASON_CATEGORY: str = "warehouse relation"
NO_CONNECTION_REASON: str = "no connection: warehouse"
SNAPSHOT_MISSING_CATEGORY: str = "snapshot missing"
SNAPSHOT_STALE_CATEGORY: str = "snapshot stale"
SNAPSHOT_INVALID_CATEGORY: str = "snapshot invalid"
FIXTURE_TYPE_CATEGORY: str = "fixture type"

# SQLQuack's help names its own project layout; SQLBuild declares sources under sql/sources/.
SQLQUACK_SOURCES_HELP: str = "a source file of the project (`sources/*.yml`)"
SQLBUILD_SOURCES_HELP: str = "a source file of the project (`sql/sources/`)"

# The session parameter SQLQuack converts for; local DuckDB sessions run in UTC.
TIMEZONE_PARAMETER: str = "TIMEZONE"
SESSION_PARAMETERS_KEY: str = "session_parameters"
UTC_TIME_ZONE: str = "UTC"

# Refused statements without a construct name the whole statement.
STATEMENT_CONSTRUCT: str = "statement"

# Result codes of tests the run could not place: refused locally, or with no warehouse to run on.
CANNOT_RUN_LOCALLY_CODE: str = "T007"
NEEDS_WAREHOUSE_CODE: str = "T008"
CANNOT_RUN_LOCALLY_MESSAGE: str = "can't run locally"
NEEDS_WAREHOUSE_MESSAGE: str = "needs the warehouse (no connection)"
