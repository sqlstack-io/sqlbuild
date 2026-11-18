"""Text that marks a native SQL parse using a library's default complexity guard."""

CRATES_DIRECTORY: str = "crates"
SOURCE_SUFFIX: str = ".rs"
SKIPPED_DIRECTORIES: frozenset[str] = frozenset({"tests", "target", "parse_guard"})
SKIPPED_FILES: frozenset[str] = frozenset({"tests.rs"})
DEFAULT_GUARD_PATTERNS: tuple[str, ...] = (
    r"ParseOptions::default\(\)",
    r"complexity_guard: None",
    r"maxFunctionCallDepth",
    r"max_function_call_depth: Some\((?!MAX_FUNCTION_CALL_DEPTH\))",
    r"const MAX_FUNCTION_CALL_DEPTH",
    r"polyglot_sql::(?:parse|transpile|validate)\(",
    r"Dialect::get(?:_by_name)?\([^;]*?\)\s*(?:\.[a-z_]+\([^;]*?\)\??\s*)*?\.parse\(",
)
GUARDED_OPTION_LITERALS: tuple[str, ...] = (
    "SchemaValidationOptions {",
    "AnalyzeQueryOptions {",
    "ValidationOptions {",
)
GUARD_FIELD: str = "complexity_guard"
TYPE_POSITION_SUFFIXES: tuple[str, ...] = ("->", "struct")
