"""DuckDB differential constants."""

from __future__ import annotations

from pathlib import Path

HARNESS_MANIFEST: Path = Path(__file__).parent / "harness" / "Cargo.toml"
HARNESS_BINARY: str = "duckdb-differential"
SQLQUACK_CRATES: tuple[str, ...] = ("sqlquack-syntax", "sqlquack-syntax-duckdb")
REPO_ROOT: Path = Path(__file__).parents[2]
GOLDENS_DIR: Path = REPO_ROOT / "tests" / "goldens" / "compiler"
GOLDEN_CORPORA: tuple[str, ...] = ("seed", "example", "fixture", "failure")
COMPILED_PROJECT_CAPTURE: str = "captures/0-compile/003-compiled_project.json"
DUCKDB_DIALECT: str = "duckdb"
DUCKDB_ADAPTER: str = "duckdb"
# Adapters whose statements run on DuckDB.
DUCKDB_ADAPTERS: tuple[str, ...] = ("duckdb", "motherduck")
RECORD_DIR_VARIABLE: str = "SQLBUILD_DUCKDB_DIFFERENTIAL_RECORD_DIR"
CONFIG_FILE: str = "sqlbuild_project.toml"
ADAPTER_SETTING: str = r'adapter = "(\w+)"'
SOURCE_SEPARATOR: str = "="

SHARED_NODES_KEY: str = "__shared_nodes__"
SHARED_KEY: str = "__shared__"
CAPTURE_KEY: str = "__capture__"

# SQLBuild's syntax gate: Polyglot `validate` for model queries and hooks, `parse_one` for
# function bodies; SQL it emits or binds without that gate is `none`.
VALIDATE_MODE: str = "validate"
PARSE_MODE: str = "parse"
NO_GATE: str = "none"
# The compiled tree's top directory and the body kind it holds.
COMPILED_KINDS: dict[str, str] = {
    "models": "model",
    "audits": "audit",
    "tests": "sql_test",
    "functions": "function_ddl",
}
CATEGORIES: tuple[str, ...] = ("both", "polyglot_only", "sqlquack_only", "neither")
EXAMPLES_PER_GROUP: int = 5
