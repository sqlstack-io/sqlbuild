"""Stable constants for SQLBuild custom Fensu rules."""

from fensu import RuleOption

ADAPTER_CLASS_BOUNDARIES: frozenset[str] = frozenset(
    {"_helpers", "classes", "models", "types", "constants", "exceptions", "shared"}
)
ADAPTER_CLASS_BYPASS_MODULES: frozenset[str] = frozenset(
    {"main.py", "models.py", "types.py", "constants.py", "exceptions.py", "helpers.py"}
)
ALLOWED_COMMENT_PREFIXES: tuple[str, ...] = (
    "#!",
    "# -*-",
    "# coding:",
    "# noqa",
    "# type: ignore",
    "# pyright:",
    "# pylint:",
    "# pragma:",
    "# sc: allow-param-mutation",
)
ALLOWED_MACRO_LOAD_PATHS: frozenset[str] = frozenset(
    {
        "src/sqlbuild/compiler/compile/_helpers/render/user_macros.py",
    }
)
ALLOWED_PARAMETER_MUTATION_COMMENT: str = "# sc: allow-param-mutation"
ALLOWED_SOURCE_FRESHNESS_INSERT_PREFIXES: tuple[str, ...] = (
    "scripts/fensu_policy/",
    "src/sqlbuild/adapter/",
    "src/sqlbuild/adapters/",
)
ALLOWED_DBT_REF_SCAN_PATHS: frozenset[str] = frozenset(
    {
        "src/sqlbuild/integrations/dbt/_helpers/manifest/sqlbuild_refs.py",
        "src/sqlbuild/integrations/dbt/_helpers/manifest/compile_refs.py",
    }
)
ALLOWED_METADATA_LOOP_PATHS: frozenset[str] = frozenset(
    {
        "src/sqlbuild/adapter/relations/main/relation_lookup.py",
        "src/sqlbuild/compiler/planner/_helpers/warehouse/semantic_sources.py",
        "src/sqlbuild/executor/janitor/_helpers/plan.py",
        "src/sqlbuild/executor/pipeline/_helpers/testing.py",
        "src/sqlbuild/executor/run/_helpers/materializations/microbatch.py",
        "src/sqlbuild/integrations/dbt/_helpers/lineage/columns.py",
        "src/sqlbuild/integrations/dbt/_helpers/planning/model_planning.py",
    }
)
CLIENT_STYLE_PREFIXES: frozenset[tuple[str, ...]] = frozenset(
    {("src", "sqlbuild", "adapters"), ("src", "sqlbuild", "integrations")}
)
DEV_TOOLING_FILE_PREFIXES: tuple[str, ...] = ("check_", "format_", "lint_", "test_")
DEV_TOOLING_SEGMENTS: frozenset[str] = frozenset({"checks", "tooling"})
DISCARDED_CALL_ALLOWED_NAMES: frozenset[str] = frozenset({"print"})
DISCARDED_CALL_ALLOWED_PREFIXES: tuple[str, ...] = (
    "check_",
    "enforce_",
    "validate_",
    "on_",
    "report_",
    "log",
    "write_",
)
FORBIDDEN_GENERIC_FILENAMES: frozenset[str] = frozenset(
    {"base.py", "common.py", "helpers.py", "misc.py"}
)
GLOBAL_REUSE_FORBIDDEN_TERMS: tuple[str, ...] = (
    "source_fingerprint",
    "source_target_name",
    "source_connection",
    "target_cursor",
    "REUSE_RELATION",
    "reuse_relation",
)
GRAPH_KEY_CLASS_NAMES: frozenset[str] = frozenset({"GraphNodeKey", "SelectionStalenessNodeKey"})
MAIN_SUPPORT_PACKAGE_NAMES: frozenset[str] = frozenset({"_helpers", "classes", "shared"})
REUSE_FORBIDDEN_TERMS: tuple[str, ...] = (
    "source_relation",
    "source relation",
    "source/target",
    "source_cursor",
    "target_relation",
    "target relation",
    "target_cursor",
)
REUSE_PATH_MARKERS: tuple[str, ...] = (
    "direct_reuse",
    "reuse_candidates.py",
    "reuse_execute.py",
    "reuse_plan.py",
    "reuse.py",
)
REUSE_TERM_ALLOWED_PATHS: dict[str, frozenset[str]] = {
    "source_target_name": frozenset(
        {"src/sqlbuild/compiler/planner/_helpers/warehouse/source_deferral.py"}
    ),
}
SELECTOR_STRING_METHOD_NAMES: frozenset[str] = frozenset(
    {"startswith", "endswith", "lstrip", "rstrip"}
)
SOURCE_FRESHNESS_MARKERS: tuple[str, ...] = ("SOURCE_FRESHNESS", "source_freshness")
SOURCE_FRESHNESS_SINGULAR_WRITER: str = "write_source_freshness_record"
WAREHOUSE_METADATA_METHODS: frozenset[str] = frozenset(
    {
        "list_relations",
        "relation_exists",
        "get_columns",
        "get_all_columns",
        "describe_relation",
        "schema_exists",
        "get_table_freshness_metadata",
        "get_tables_freshness_metadata",
        "query_column_names",
        "list_functions",
    }
)

ABSTRACT_METHOD_DECORATOR_NAME: str = "abstractmethod"
ABC_MODULE_NAME: str = "abc"
ADAPTER_CLASS_MODULE_MIN_PARTS: int = 6
ADAPTER_CONTRACT_CLASSES_PATH: str = "src/sqlbuild/adapter/contract/classes"
ADAPTER_BUILTINS_PATH: str = "src/sqlbuild/adapter/discovery/main/builtins.py"
ADAPTER_IMPLEMENTATION_PATH_MARKERS: tuple[str, str] = ("/adapters/", "/classes/")
ADAPTER_ROOT_PARTS: tuple[str, ...] = ("src", "sqlbuild", "adapter")
ADAPTERS_ROOT_PARTS: tuple[str, ...] = ("src", "sqlbuild", "adapters")
ADAPTER_PACKAGE_IMPORT_PREFIX: str = "sqlbuild.adapters."
ADAPTER_CONTRACT_BASE_SYMBOLS: frozenset[tuple[str, ...]] = frozenset(
    {
        (
            "sqlbuild",
            "adapter",
            "contract",
            "classes",
            "base_adapter",
            "BaseAdapter",
        ),
        (
            "sqlbuild",
            "adapter",
            "contract",
            "classes",
            "duckdb_backed_adapter",
            "DuckDbBackedAdapter",
        ),
        (
            "sqlbuild",
            "adapter",
            "contract",
            "classes",
            "strict_adapter",
            "StrictAdapter",
        ),
    }
)
ADAPTER_PUBLIC_EXECUTE_OWNER: str = "src/sqlbuild/adapter/contract/classes/connection.py"
ADAPTER_PUBLIC_EXECUTE_NAME: str = "execute"
BASE_ADAPTER_CLASS_NAME: str = "BaseAdapter"
BASE_ADAPTER_REFERENCE_PART_COUNT: int = 2
CLIENT_MODULE_MIN_PARTS: int = 5
CLIENT_MODULE_NAME: str = "client.py"
CLASSES_PACKAGE_NAME: str = "classes"
COMPILER_EXECUTOR_DOMAIN_NAMES: frozenset[str] = frozenset({"compiler", "executor"})
DBT_INTEGRATION_PATH_PREFIX: str = "src/sqlbuild/integrations/dbt/"
DBT_REF_ATTRIBUTE_NAME: str = "DBT_REF"
HELPERS_PACKAGE_NAME: str = "_helpers"
INIT_MODULE_NAME: str = "__init__.py"
INSERT_SQL_PREFIX: str = "INSERT INTO"
LEGACY_DUCKDB_ADAPTER_SUFFIX: tuple[str, ...] = ("shared", "classes", "duckdb.py")
DUCKDB_BACKED_ADAPTER_PATH: str = "src/sqlbuild/adapter/contract/classes/duckdb_backed_adapter.py"
LOAD_PROJECT_MACROS_NAME: str = "load_project_macros"
NAME_REFERENCE_KIND: str = "name"
MAIN_MODULE_NAME: str = "main.py"
MAIN_PACKAGE_NAME: str = "main"
NESTED_HELPER_MODULE_MIN_PARTS: int = 5
PLANNER_PATH_PREFIX: str = "src/sqlbuild/compiler/planner/"
POLICY_EVALUATION_SCOPES: frozenset[str] = frozenset({"root", "tooling"})
POLICY_IMPLEMENTATION_PATH_PREFIX: str = "scripts/fensu_policy/"
PROVIDER_MODULE_PARTS: tuple[str, ...] = ("src", "sqlbuild", "providers.py")
PUBLIC_EXPORT_ASSIGNMENT_TARGETS: tuple[str, ...] = ("__all__",)
ROOT_SCOPE_NAME: str = "root"
RUNTIME_ROOT_PARTS: tuple[str, ...] = ("src", "sqlbuild")
SELECTOR_MARKER: str = "+"
STRING_LITERAL_KIND: str = "string"
SQL_REFERENCE_KIND_CLASS_NAME: str = "SqlReferenceKind"
STRICT_ADAPTER_CLASS_NAME: str = "StrictAdapter"
ATTRIBUTE_REFERENCE_KIND: str = "attribute"

APPROVED_RAW_STATEMENT_MODULES: frozenset[str] = frozenset(
    {
        "src/sqlbuild/adapter/contract/classes/observed_connection.py",
        "src/sqlbuild/adapter/contract/classes/observed_cursor.py",
        "src/sqlbuild/adapters/bigquery/classes/bigquery_connection.py",
        "src/sqlbuild/adapters/clickhouse/classes/clickhouse_connection.py",
        "src/sqlbuild/adapters/databricks/classes/databricks_connection.py",
        "src/sqlbuild/adapters/postgres/classes/postgres_connection.py",
        "src/sqlbuild/adapters/snowflake/classes/snowflake_connection.py",
        "src/sqlbuild/adapters/snowflake/classes/snowflake_cursor.py",
        "src/sqlbuild/adapters/sqlserver/classes/sqlserver_connection.py",
    }
)
KNOWN_DRIVER_MODULE_PREFIXES: tuple[tuple[str, ...], ...] = (
    ("duckdb",),
    ("psycopg",),
    ("snowflake", "connector"),
    ("google", "cloud", "bigquery"),
    ("databricks", "sql"),
    ("pyodbc",),
)
RAW_DRIVER_FACTORY_NAMES: frozenset[str] = frozenset({"Client", "connect", "connect_to_cloud"})
RAW_CURSOR_FACTORY_NAME: str = "cursor"
RAW_STATEMENT_METHOD_NAMES: frozenset[str] = frozenset({"execute", "executemany", "query"})

LIFECYCLE_CONSTRUCTION_OWNER_PATHS: frozenset[str] = frozenset(
    {
        "src/sqlbuild/runtime/observability/_helpers/factory.py",
        "src/sqlbuild/runtime/observability/_helpers/observability.py",
    }
)
LIFECYCLE_FACTORY_OWNER_PATHS: frozenset[str] = frozenset(
    {
        "src/sqlbuild/cli/commands/main/entrypoint/_dispatch_with_observability.py",
        "src/sqlbuild/executor/pipeline/main/run.py",
        "src/sqlbuild/observability.py",
        "src/sqlbuild/runtime/observability/classes/microbatch_lifecycle.py",
        "src/sqlbuild/runtime/observability/classes/operation_lifecycle.py",
        "src/sqlbuild/runtime/observability/classes/resource_attempt_lifecycle.py",
        "src/sqlbuild/runtime/observability/classes/run_lifecycle.py",
        "src/sqlbuild/runtime/observability/classes/statement_lifecycle.py",
        "src/sqlbuild/runtime/observability/main/create_lifecycle_event.py",
    }
)
LIFECYCLE_CATALOG_OWNER_PATH: str = "src/sqlbuild/runtime/observability/constants.py"
DIAGNOSTIC_CONSTRUCTION_OWNER_PATHS: frozenset[str] = frozenset(
    {
        "src/sqlbuild/runtime/compute_logs/classes/diagnostic_log_handler.py",
        "src/sqlbuild/runtime/observability/_helpers/observability.py",
    }
)

EVENT_EXPORTER_RUNTIME_PREFIX: str = "src/sqlbuild/runtime/event_exporting/"
EVENT_EXPORTER_PUBLIC_PATH: str = "src/sqlbuild/sinks.py"
EVENT_EXPORTER_DIRECTORY_NAME: str = "sinks"
EVENT_EXPORTER_MODULE_PARTS: tuple[str, ...] = ("sqlbuild", "runtime", "event_exporting")
CORE_EVENT_EXPORTER_CLASS_NAMES: frozenset[str] = frozenset(
    {
        "EventExporterCommandScope",
        "EventExporterDispatcher",
        "FinitePriorityEventQueue",
        "MutableEventExporterCounts",
    }
)
EVENT_EXPORTER_INTEGRATION_PATHS: frozenset[str] = frozenset(
    {
        "src/sqlbuild/cli/commands/_helpers/entry/observability.py",
        "src/sqlbuild/compiler/discovery/_helpers/filesystem/project_modules.py",
        "src/sqlbuild/compiler/discovery/_helpers/filesystem/command_output_sinks.py",
        "src/sqlbuild/compiler/discovery/_helpers/yml/project.py",
        "src/sqlbuild/compiler/discovery/main/runtime_extensions.py",
        "src/sqlbuild/compiler/discovery/models.py",
        "src/sqlbuild/provider/_helpers/session.py",
    }
)
LIFECYCLE_DEFINITION_CLASS_NAME: str = "LifecycleEventDefinition"
LIFECYCLE_DEFINITION_FACTORY_NAME: str = "create"
LIFECYCLE_CATALOG_NAME_PREFIX: str = "LIFECYCLE_EVENT_CATALOG"
LIFECYCLE_EVENT_CLASS_NAME: str = "LifecycleEvent"
DIAGNOSTIC_LOG_CLASS_NAME: str = "DiagnosticLog"
LIFECYCLE_EVENT_FACTORY_NAME: str = "create_lifecycle_event"
EVENT_EXPORTER_DECORATOR_NAMES: frozenset[str] = frozenset(
    {"lifecycle_event_sink", "command_output_sink"}
)
OBSERVABILITY_LIFECYCLE_EVENT_MODULES: tuple[tuple[str, ...], ...] = (
    ("sqlbuild", "observability"),
    ("sqlbuild", "runtime", "observability", "models"),
)
OBSERVABILITY_FACTORY_MODULES: tuple[tuple[str, ...], ...] = (
    ("sqlbuild", "observability"),
    ("sqlbuild", "runtime", "observability", "_helpers", "factory"),
    ("sqlbuild", "runtime", "observability", "main", "create_lifecycle_event"),
)
OBSERVABILITY_DEFINITION_MODULES: tuple[tuple[str, ...], ...] = (
    ("sqlbuild", "runtime", "observability", "models"),
)
PUBLIC_EVENT_EXPORTER_MODULE_PARTS: tuple[str, ...] = ("sqlbuild", "sinks")
NATIVE_BINDINGS_ROOT: str = "crates/sqlbuild-python/src/"
NATIVE_CROSSING_PATTERNS: tuple[str, ...] = (
    r"\.getattr\(",
    r"\.setattr\(",
    r"\.call_method[01]?\(",
    r"\.call[01]?\(",
    r"\bregistry::(?:native|error)\(",
    r"\bimported\(",
    r"\.import\(",
    r"\bPyModule::import\(",
    r"\"dataclasses\"",
    r"\"sqlbuild\.",
)
NATIVE_PYTHON_FIELD_PATTERN: str = (
    r"\b(?:Py|Bound|Borrowed)<[^;]*?\b(?:PyAny|PyDict|PyList|PyTuple|PySet|PyFrozenSet"
    r"|PyString|PyType|PyModule)\b"
)
NATIVE_TEST_MODULE_MARKER: str = "#[cfg(test)]"
NATIVE_RUST_SUFFIX: str = ".rs"
NATIVE_TEST_DIRECTORY: str = "tests"
NATIVE_TEST_FILE_NAME: str = "tests.rs"
NATIVE_LINE_COMMENT: str = "//"
NATIVE_STRING_QUOTE: str = '"'
NATIVE_SKIPPED_LINE_PREFIXES: tuple[str, ...] = (
    "//",
    "use ",
    "pub use ",
    "pub(crate) use ",
    "const ",
    "pub(crate) const ",
    "pub const ",
)
NATIVE_BUDGET_SEPARATOR: str = "="
NATIVE_EDGE_PATHS: RuleOption[tuple[str, ...]] = RuleOption.string_list(
    name="edge_paths",
    default=(),
    description="Path prefixes that own the real Python edges and may cross freely.",
)
NATIVE_PENDING_BUDGETS: RuleOption[tuple[str, ...]] = RuleOption.string_list(
    name="pending_budgets",
    default=(),
    description="`path=count` budgets for files that still cross; each may only fall.",
)
NATIVE_HELPERS_ROOT: str = "crates/sqlbuild-python/src/bindings/_helpers/"
NATIVE_HELPERS_PATH: str = "crate::bindings::_helpers::"
NATIVE_MODULE_FILE_STEM: str = "mod"
NATIVE_MODULE_PATH_SEPARATOR: str = "/"
NATIVE_LAYER_SIBLING_SEPARATOR: str = "|"
NATIVE_LAYER_WILDCARD: str = "*"
NATIVE_GROUP_OPEN: str = "{"
NATIVE_GROUP_CLOSE: str = "}"
NATIVE_GROUP_SEPARATOR: str = ","
NATIVE_PATH_SEPARATOR: str = "::"
NATIVE_PATH_SELF: str = "self"
NATIVE_LAYER_ORDER: RuleOption[tuple[str, ...]] = RuleOption.string_list(
    name="layer_order",
    default=(),
    description=(
        "Helper module layers from the bottom up; `a|b` makes sibling layers that may not "
        "import each other."
    ),
)
NATIVE_MODULE_LAYERS: RuleOption[tuple[str, ...]] = RuleOption.string_list(
    name="module_layers",
    default=(),
    description=(
        "`module/path=layer` assignments of helper modules under bindings/_helpers; the longest "
        "matching path decides."
    ),
)
NATIVE_MODULE_PATH: str = "sqlbuild._native"
NATIVE_MODULE_PACKAGE: str = "sqlbuild"
NATIVE_MODULE_NAME: str = "_native"
NATIVE_PATCH_CALL_NAMES: frozenset[str] = frozenset({"setattr", "object", "delattr"})
NATIVE_STRING_PATCH_CALL_NAMES: frozenset[str] = frozenset({"setattr", "patch", "delattr"})
NATIVE_TEST_PATH_PREFIX: str = "tests/"
NATIVE_TEST_PATCH_BUDGETS: RuleOption[tuple[str, ...]] = RuleOption.string_list(
    name="pending_budgets",
    default=(),
    description="`path=count` budgets for test files that still patch the native module.",
)
BRACE_CLOSE: str = "}"
BRACE_OPEN: str = "{"
BRACKET_CLOSE: str = "]"
BRACKET_OPEN: str = "["
CONTROL_STORE_SCHEMA_PATHS: RuleOption[tuple[str, ...]] = RuleOption.string_list(
    name="schema_paths",
    default=(),
    description="Repository-relative directories whose *.sql files create control-store state.",
)
LIST_SEPARATOR: str = ","
PARENTHESIS_CLOSE: str = ")"
PARENTHESIS_OPEN: str = "("
REMOVED_CODE: str = "\0"
RUST_ATTRIBUTE_PATTERN: str = r"#\s*\["
RUST_BARE_WORD_PATTERN: str = r"(?<![:\w])([A-Za-z_]\w*)\b(?!\s*::)"
RUST_ENUM_PATTERN: str = r"\benum\s+(?P<name>[A-Za-z_]\w*)\s*(?:<[^{;]*>)?\s*(?:where\b[^{;]*)?\{"
RUST_FROM_ATTRIBUTE_PATTERN: str = r"#\s*\[\s*from\s*\]"
RUST_IMPL_HEADER_PATTERN: str = r"\bimpl\b[^{;]*\{"
RUST_LINE_CONTINUATION: str = r"\\\n\s*"
RUST_LITERAL_PATTERN: str = (
    r"//[^\n]*"
    r"|/\*.*?\*/"
    r"|(?P<raw>b?r(?P<hashes>#*)\"(?P<raw_body>.*?)\"(?P=hashes))"
    r"|(?P<plain>b?\"(?P<plain_body>(?:\\.|[^\"\\])*)\")"
    r"|'(?:\\u\{[0-9a-fA-F]+\}|\\.|[^\\'\n])'"
)
RUST_MACRO_INVOCATION_PATTERN: str = r"\b[A-Za-z_]\w*!\s*\{"
RUST_PATH_PAIR_PATTERN: str = r"(?=\b([A-Za-z_]\w*)\s*::\s*([A-Za-z_]\w*)\b)"
RUST_SELF_PATH_PATTERN: str = r"\bSelf\s*::\s*([A-Za-z_]\w*)\b"
RUST_SIMPLE_ESCAPES: dict[str, str] = {
    "n": "\n",
    "t": "\t",
    "r": "\r",
    "0": "\0",
    "\\": "\\",
    '"': '"',
    "'": "'",
}
RUST_TEST_FILE_NAME: str = "tests.rs"
RUST_TEST_MODULE_PATTERN: str = (
    r"#\s*\[\s*cfg\s*\(\s*test\s*\)\s*\]\s*(?:pub(?:\([^)]*\))?\s+)?mod\s+\w+\s*\{"
)
RUST_TEST_PATH_SEGMENTS: frozenset[str] = frozenset({"tests", "benches", "examples"})
RUST_VARIANT_IMPORT_PATTERN: str = r"\b([A-Za-z_]\w*)\s*::\s*(?:\*|\{)"
SQL_ADD_COLUMN_PATTERN: str = (
    r"\bALTER\s+TABLE\s+(?:IF\s+EXISTS\s+)?(?:ONLY\s+)?\"?(\w+)\"?\s+ADD\s+(?:COLUMN\s+)?"
    r"(?:IF\s+NOT\s+EXISTS\s+)?\"?(\w+)\"?"
)
SQL_ASSIGNMENT: str = "="
SQL_COLUMN_CONSTRAINT_WORDS: frozenset[str] = frozenset(
    {"constraint", "primary", "unique", "foreign", "check", "exclude", "like"}
)
SQL_COMMENT_PATTERN: str = r"--[^\n]*|/\*.*?\*/"
SQL_CONFLICT_TARGET_WORDS: frozenset[tuple[str, str]] = frozenset({("conflict", "on")})
SQL_CREATE_TABLE_PATTERN: str = (
    r"\bCREATE\s+(?:UNLOGGED\s+)?TABLE\s+(?:IF\s+NOT\s+EXISTS\s+)?\"?(\w+)\"?\s*\("
)
SQL_DDL_PREFIX_PATTERN: str = r"\s*(?:CREATE|ALTER|DROP|COMMENT|GRANT|REVOKE)\b"
SQL_DELETE_WORD: str = "delete"
SQL_EXCLUDED_QUALIFIER: str = "excluded"
SQL_FILE_PATTERN: str = "*.sql"
SQL_FRAGMENT_MARKERS: frozenset[str] = frozenset({"=", ",", "is", "in"})
SQL_FROM_WORD: str = "from"
SQL_NEUTRAL_WORDS: frozenset[str] = frozenset(
    {"and", "or", "not", "is", "null", "in", "as", "asc", "desc", "true", "false"}
)
SQL_QUALIFIER: str = "."
SQL_READ_WORDS: frozenset[str] = frozenset(
    {"select", "from", "join", "on", "where", "group", "order", "having", "returning", "using"}
)
SQL_SET_WORD: str = "set"
SQL_STAR: str = "*"
SQL_STAR_PREDECESSORS: frozenset[str] = frozenset({"select", ",", "."})
SQL_STATEMENT_SEPARATOR: str = ";"
SQL_TOKEN_PATTERN: str = r"\"[^\"]*\"|'(?:[^']|'')*'|[A-Za-z_]\w*|[(),.*=]"
SQL_VIEW_PREFIX_PATTERN: str = r"\s*CREATE\s+(?:OR\s+REPLACE\s+)?(?:MATERIALIZED\s+)?VIEW\b"
SQL_WORD_PATTERN: str = r"[A-Za-z_]\w*"
SQL_WRITE_WORDS: frozenset[str] = frozenset(
    {"insert", "into", "values", "update", "set", "delete", "conflict", "do"}
)
