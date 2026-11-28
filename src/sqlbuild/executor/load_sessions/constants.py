"""Adapter names whose sessions need more than SQL statements."""

DUCKDB_BACKED_ADAPTERS: frozenset[str] = frozenset({"duckdb", "motherduck"})
SNOWFLAKE_ADAPTER: str = "snowflake"
POSTGRES_ADAPTER: str = "postgres"
BIGQUERY_ADAPTER: str = "bigquery"
CLICKHOUSE_ADAPTER: str = "clickhouse"
