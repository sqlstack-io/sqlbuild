"""Scenario execution and local replay constants."""

import re

from sqlbuild.adapter.contract.types import TypeDialect

SCENARIO_EXEC_FIXTURE_FAILED: str = "X501"
SCENARIO_EXEC_SEED_FAILED: str = "X502"
SCENARIO_EXEC_MODEL_FAILED: str = "X503"
SCENARIO_EXEC_EXPECTED_ERRORED: str = "X504"
SCENARIO_EXEC_EXPECTED_INTERNAL: str = "X505"
SCENARIO_EXEC_EXPECTED_FAILED: str = "X506"
SCENARIO_EXEC_ASSERTION_ERRORED: str = "X507"
SCENARIO_EXEC_ASSERTION_FAILED: str = "X508"
SCENARIO_EXEC_CLEANUP_FAILED: str = "X509"
SCENARIO_EXEC_CAPTURE_FAILED: str = "X510"
SCENARIO_EXEC_CAPTURE_INTERNAL: str = "X511"
SCENARIO_EXEC_CAPTURE_LIMIT_EXCEEDED: str = "X512"
SCENARIO_EXEC_INTERNAL: str = "X599"
SCENARIO_LOCAL_SNAPSHOT_MISSING: str = "X601"
SCENARIO_LOCAL_SNAPSHOT_STALE: str = "X602"
SCENARIO_LOCAL_MANIFEST_INVALID: str = "X603"
SCENARIO_LOCAL_JSONL_INVALID: str = "X604"
SCENARIO_LOCAL_TYPE_INVALID: str = "X605"
SCENARIO_LOCAL_LOAD_FAILED: str = "X606"
SCENARIO_LOCAL_SQL_TRANSPILE_FAILED: str = "X607"
SCENARIO_LOCAL_MODEL_FAILED: str = "X608"
SCENARIO_LOCAL_FUNCTION_FAILED: str = "X609"
SCENARIO_LOCAL_INTERNAL: str = "X699"

LOCAL_TYPE_BIGINT: str = "BIGINT"
LOCAL_TYPE_BOOLEAN: str = "BOOLEAN"
LOCAL_TYPE_DATE: str = "DATE"
LOCAL_TYPE_DECIMAL: str = "DECIMAL"
LOCAL_TYPE_DOUBLE: str = "DOUBLE"
LOCAL_TYPE_INT: str = "INT"
LOCAL_TYPE_INT128: str = "INT128"
LOCAL_TYPE_JSON: str = "JSON"
LOCAL_TYPE_REAL: str = "REAL"
LOCAL_TYPE_SMALLINT: str = "SMALLINT"
LOCAL_TYPE_TEXT: str = "TEXT"
LOCAL_TYPE_FLOAT: str = "FLOAT"
LOCAL_TYPE_TIMETZ: str = "TIMETZ"
LOCAL_TYPE_VARIANT: str = "VARIANT"
LOCAL_TYPE_TIME: str = "TIME"
LOCAL_TYPE_TIMESTAMP: str = "TIMESTAMP"
LOCAL_TYPE_TIMESTAMPTZ: str = "TIMESTAMPTZ"
LOCAL_TYPE_VARIANT: str = "VARIANT"
LOCAL_TYPE_UINT128: str = "UINT128"
LOCAL_TYPE_VARCHAR: str = "VARCHAR"

POSTGRES_SERIAL_TYPES: frozenset[str] = frozenset({"BIGSERIAL", "SERIAL", "SMALLSERIAL"})
POSTGRES_BIGSERIAL_TYPE: str = "BIGSERIAL"
POSTGRES_SMALLSERIAL_TYPE: str = "SMALLSERIAL"
POSTGRES_PARAMETERIZED_TEXT_TYPES: frozenset[str] = frozenset({"VARCHAR", "CHARACTER VARYING"})
POSTGRES_PARAMETERIZED_DECIMAL_TYPES: frozenset[str] = frozenset({"NUMERIC", "DECIMAL"})
POSTGRES_VARCHAR_TYPES: frozenset[str] = frozenset(
    {
        "CIDR",
        "CIRCLE",
        "DATERANGE",
        "INT4RANGE",
        "INT8RANGE",
        "LINE",
        "LSEG",
        "MACADDR",
        "MACADDR8",
        "NAME",
        "NUMRANGE",
        "PATH",
        "POINT",
        "POLYGON",
        "REGCLASS",
        "TSQUERY",
        "TSRANGE",
        "TSTZRANGE",
        "TSVECTOR",
        "VARBIT",
    }
)

SNOWFLAKE_NUMBER_TYPE: str = "NUMBER"
SNOWFLAKE_DOUBLE_TYPES: frozenset[str] = frozenset(
    {"FLOAT", "FLOAT8", "DOUBLE", "DOUBLE PRECISION"}
)
SNOWFLAKE_REAL_TYPE: str = "FLOAT4"
SNOWFLAKE_TIMEZONE_TIMESTAMP_TYPES: frozenset[str] = frozenset({"TIMESTAMP_LTZ", "TIMESTAMP_TZ"})
SNOWFLAKE_TIMESTAMP_NTZ_TYPE: str = "TIMESTAMP_NTZ"
SNOWFLAKE_VECTOR_TYPE: str = "VECTOR"
BIGQUERY_BIGNUMERIC_TYPE: str = "BIGNUMERIC"
BIGQUERY_VARCHAR_TYPES: frozenset[str] = frozenset({"RANGE", "BYTES"})
BIGQUERY_TIMESTAMP_TYPE: str = "TIMESTAMP"
BIGQUERY_WIDEN_TO_DOUBLE_TYPES: frozenset[str] = frozenset({"FLOAT64"})
DATABRICKS_TIMESTAMP_TYPE: str = "TIMESTAMP"
DATABRICKS_TIMESTAMP_NTZ_TYPE: str = "TIMESTAMP_NTZ"
DATABRICKS_VOID_TYPE: str = "VOID"
DATABRICKS_INTERVAL_PREFIX: str = "INTERVAL"
DATABRICKS_VARIANT_TYPE: str = "VARIANT"
DATABRICKS_ARRAY_TYPE: str = "ARRAY"
DATABRICKS_MAP_TYPE: str = "MAP"
DATABRICKS_STRUCT_TYPE: str = "STRUCT"
DATABRICKS_NESTED_TYPES: frozenset[str] = frozenset({"ARRAY", "MAP", "STRUCT"})
DATABRICKS_MAP_ARGUMENT_COUNT: int = 2
DATABRICKS_STRUCT_FIELD_PATTERN: re.Pattern[str] = re.compile(
    r"\s*(?P<name>`[^`]+`|[A-Za-z_][A-Za-z0-9_]*)\s*(?::\s*|\s+)(?P<type>\S.*?)\s*"
)
DUCKDB_HUGEINT_TYPE: str = "HUGEINT"
DUCKDB_UHUGEINT_TYPE: str = "UHUGEINT"
DUCKDB_BLOB_TYPE: str = "BLOB"
DUCKDB_SPATIAL_TYPES: frozenset[str] = frozenset({"GEOMETRY", "GEOGRAPHY"})
DUCKDB_LIST_TYPE: str = "LIST"
DUCKDB_INTEGER_TYPE: str = "INTEGER"

GENERIC_LOCAL_BASE_TYPES: frozenset[str] = frozenset(
    {
        "BOOL",
        "BOOLEAN",
        "TINYINT",
        "SMALLINT",
        "INT",
        "INTEGER",
        "BIGINT",
        "FLOAT",
        "DOUBLE",
        "DECIMAL",
        "NUMERIC",
        "DATE",
        "TIME",
        "TIMESTAMP",
        "DATETIME",
        "VARCHAR",
        "CHAR",
        "TEXT",
        "STRING",
        "JSON",
    }
)
TYPE_TREE_KIND: str = "data_type"
TYPE_TREE_LOCAL_TYPES: dict[str, str] = {
    "boolean": "BOOLEAN",
    "tiny_int": "TINYINT",
    "small_int": "SMALLINT",
    "int": "INT",
    "big_int": "BIGINT",
    "int128": "HUGEINT",
    "uint8": "UTINYINT",
    "uint16": "USMALLINT",
    "uint32": "UINTEGER",
    "uint64": "UBIGINT",
    "uint128": "UHUGEINT",
    "double": "DOUBLE",
    "date": "DATE",
    "interval": "INTERVAL",
    "json": "JSON",
    "json_b": "JSON",
    "uuid": "UUID",
    "binary": "VARCHAR",
    "var_binary": "VARCHAR",
    "blob": "VARCHAR",
    "geography": "VARCHAR",
    "geometry": "VARCHAR",
    "object": "JSON",
}
TYPE_TREE_TEXT_KINDS: frozenset[str] = frozenset({"char", "var_char", "string", "text"})
TYPE_TREE_FLOAT: str = "float"
TYPE_TREE_DECIMAL: str = "decimal"
TYPE_TREE_TIME: str = "time"
TYPE_TREE_TIMESTAMP: str = "timestamp"
TYPE_TREE_COLLECTIONS: frozenset[str] = frozenset({"array", "list"})
TYPE_TREE_STRUCT: str = "struct"
TYPE_TREE_MAP: str = "map"
STRUCT_FIELD_NAME_PATTERN: re.Pattern[str] = re.compile(r"[A-Za-z_][A-Za-z0-9_]*")
LOCAL_TYPE_DIALECTS: frozenset[TypeDialect] = frozenset(
    {TypeDialect.DUCKDB, TypeDialect.MOTHERDUCK}
)

FALLBACK_BOOLEAN_TYPES: frozenset[str] = frozenset({"BOOL", "BOOLEAN"})
FALLBACK_SIGNED_INTEGER_LOCAL_TYPES: dict[str, str] = {
    "TINYINT": "TINYINT",
    "SMALLINT": "SMALLINT",
    "INT": "INT",
    "INTEGER": "INT",
    "BIGINT": "BIGINT",
    "HUGEINT": "HUGEINT",
}
FALLBACK_UNSIGNED_INTEGER_TYPES: frozenset[str] = frozenset(
    {"UTINYINT", "USMALLINT", "UINTEGER", "UBIGINT", "UHUGEINT"}
)
FALLBACK_FLOAT_TYPES: frozenset[str] = frozenset({"FLOAT", "FLOAT4", "REAL"})
FALLBACK_DOUBLE_TYPES: frozenset[str] = frozenset(
    {"DOUBLE", "DOUBLE PRECISION", "FLOAT8", "FLOAT64"}
)
TSQL_LOCAL_TYPES: dict[str, str] = {
    "BIT": "BOOLEAN",
    "DATETIME2": "TIMESTAMP",
    "SMALLDATETIME": "TIMESTAMP",
    "DATETIMEOFFSET": "TIMESTAMPTZ",
    "UNIQUEIDENTIFIER": "UUID",
    "FLOAT": "DOUBLE",
    "REAL": "FLOAT",
    "MONEY": "DECIMAL(19,4)",
    "SMALLMONEY": "DECIMAL(10,4)",
}
FALLBACK_DECIMAL_TYPES: frozenset[str] = frozenset({"DECIMAL", "DEC", "NUMERIC", "NUMBER"})
FALLBACK_DATE_TYPE: str = "DATE"
FALLBACK_TIME_TYPES: frozenset[str] = frozenset({"TIME", "TIME WITH TIME ZONE", "TIMETZ"})
FALLBACK_TIMESTAMP_TOKEN: str = "TIMESTAMP"
FALLBACK_TIMESTAMP_TYPES: frozenset[str] = frozenset({"DATETIME", "TIMESTAMPTZ"})
FALLBACK_VARCHAR_TYPES: frozenset[str] = frozenset(
    {"VARCHAR", "CHAR", "CHARACTER", "TEXT", "STRING"}
)
FALLBACK_JSON_TYPES: frozenset[str] = frozenset({"JSON", "JSONB"})
FALLBACK_BINARY_TYPES: frozenset[str] = frozenset({"BLOB", "BYTEA", "BINARY", "VARBINARY"})
FALLBACK_STRUCTURED_TYPES: frozenset[str] = frozenset(
    {"ARRAY", "OBJECT", "VARIANT", "JSON", "JSONB", "MAP", "STRUCT"}
)
SNOWFLAKE_WIDEN_TO_DOUBLE_TYPES: frozenset[str] = SNOWFLAKE_DOUBLE_TYPES
GENERIC_WIDEN_TO_DOUBLE_TYPES: frozenset[str] = frozenset({"DOUBLE", "DOUBLE PRECISION", "FLOAT8"})

TYPE_ARGUMENT_OPEN_PAREN: str = "("
TYPE_ARGUMENT_CLOSE_PAREN: str = ")"
TYPE_ARGUMENT_CLOSE_ANGLE: str = ">"
TYPE_ARGUMENT_OPEN_TOKENS: frozenset[str] = frozenset({"(", "<"})
TYPE_ARGUMENT_CLOSE_TOKENS: frozenset[str] = frozenset({")", ">"})
TYPE_ARGUMENT_SEPARATOR: str = ","
TYPE_PATTERN_WILDCARD: str = "*"
JSONL_BLANK_LINE: str = ""
LOCAL_SCENARIO_DIALECT: str = "duckdb"
LOCAL_SCENARIO_DUCKDB_DIALECTS: frozenset[str] = frozenset({"duckdb", "motherduck"})
LOCAL_SCENARIO_CONVERTED_DIALECT: str = "snowflake"

LOCAL_CAPTURE_ADAPTER: str = "duckdb"
VARIANT_FROM_JSON_PLACEHOLDER: str = "CAST(CAST(? AS JSON) AS VARIANT)"
CANONICAL_JSON_FUNCTION: str = "sqb_canonical_json"
SEMI_STRUCTURED_COMPARISON_TEMPLATE: str = (
    CANONICAL_JSON_FUNCTION + "(CAST({column} AS JSON)) AS {column}"
)
SEMI_STRUCTURED_LOCAL_TYPES: frozenset[str] = frozenset({"VARIANT", "JSON"})
IDENTIFIER_QUOTE: str = '"'
