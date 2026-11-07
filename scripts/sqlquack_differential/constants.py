"""SQLQuack dialect differential constants."""

from __future__ import annotations

from pathlib import Path

HARNESS_MANIFEST: Path = Path(__file__).parent / "harness" / "Cargo.toml"
HARNESS_BINARY: str = "sqlquack-differential"
# A harness binary built elsewhere, such as with the `native-duckdb` feature; skips the build.
PREBUILT_BINARY_VARIABLE: str = "SQLQUACK_DIFFERENTIAL_BINARY"
SQLQUACK_CRATES: tuple[str, ...] = (
    "sqlquack-core",
    "sqlquack-semantics",
    "sqlquack-snowflake",
    "sqlquack-syntax",
    "sqlquack-syntax-derive",
)
COMPILED_PROJECT_CAPTURE: str = "captures/0-compile/003-compiled_project.json"
# Binding-catalog dialects the harness measures: Snowflake, DuckDB (and MotherDuck) and SQLite.
MEASURED_DIALECTS: tuple[str, ...] = ("snowflake", "duckdb", "sqlite")
SOURCE_SEPARATOR: str = "="

SHARED_NODES_KEY: str = "__shared_nodes__"
SHARED_KEY: str = "__shared__"
CAPTURE_KEY: str = "__capture__"
ENUM_KEY: str = "__enum__"
UNORDERED_MAPPING_KEY: str = "__unordered_mapping__"
UNKNOWN_TYPE: str = "UNKNOWN"

REFERENCE_RESOURCE_TYPES: dict[str, str] = {
    "REF": "MODEL",
    "SOURCE": "SOURCE",
    "SEED": "SEED",
    "TABLE_FUNCTION": "TABLE_FN",
}
TABLE_FUNCTION_PREFIX: str = "__sqlbuild_table_function_"
MEASURES: tuple[str, ...] = ("lineage", "findings", "types")
EXAMPLES_PER_GROUP: int = 5
TABLE_FUNCTION_KIND: str = "TABLE_FUNCTION"
MUTATION_SEPARATOR: str = "#"
OPAQUE_PARSE: str = "opaque"
GAP_SEPARATOR: str = ","
COMPILED_ORIGIN: str = "compiled"
MUTATED_ORIGIN: str = "mutated"
MISSING_SUFFIX: str = "_missing"
UNKNOWN_QUALIFIER: str = "zq"
COLUMN_REFERENCE: str = r'(?<![\w"\'])(?:([a-z]\w*)\.)?([a-z]\w*)(?![\w"(.])'
QUALIFIED_REFERENCE: str = r"(?<![\w\"'.])([a-z]\w*)\.([a-z]\w*)\b"
OUTPUT_ALIAS_BEFORE: str = r"\bAS\s*$"
JOIN_KEYWORD: str = r"\bJOIN\b"
