"""Constants for source loader execution."""

LOADER_RELATION_QUALIFIER_SEPARATOR: str = "."
SQLBUILD_CONTEXT_MEMBERS: frozenset[str] = frozenset(
    {
        "adapter",
        "connection_config",
        "destination",
        "destination_database",
        "destination_schema",
        "destination_name",
        "run_id",
        "target",
        "target_config",
        "vars",
        "use_color",
        "providers",
        "result_store",
        "runtime_dir",
        "execute_sql",
        "query",
        "log",
        "progress",
        "skip",
        "result",
        "qualify_name",
        "qualify_in_destination_schema",
        "loader",
        "result_of",
        "results_of",
    }
)
LOADER_SESSION: str = "sqlbuild_warehouse"
INTERMEDIATE_WRITE_STRATEGY: str = "replace"
LOADER_NAMING: str = "source"
NOT_CALLED: object = object()
