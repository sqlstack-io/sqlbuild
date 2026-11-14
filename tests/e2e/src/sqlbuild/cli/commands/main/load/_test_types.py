from dataclasses import dataclass, field


@dataclass(frozen=True)
class SourceLoaderStrategiesE2ETestCase:
    description: str
    command: tuple[str, ...]
    expected_countries: tuple[tuple[object, ...], ...]
    expected_webhook_event_counts: tuple[tuple[object, ...], ...]
    expected_order_events: tuple[tuple[object, ...], ...]
    expected_customers: tuple[tuple[object, ...], ...]
    expected_loader_status: tuple[tuple[object, ...], ...]
    expected_stdout_fragments: tuple[str, ...] = field(default_factory=tuple)
    expected_return_code: int = 0


@dataclass(frozen=True)
class SourceLoaderSchemaEvolutionE2ETestCase:
    description: str
    command: tuple[str, ...]
    expected_rows: tuple[tuple[object, ...], ...]
    expected_return_code: int = 0


@dataclass(frozen=True)
class ChainedLoaderSelectionE2ETestCase:
    description: str
    command: tuple[str, ...]
    expected_raw_events_exists: bool
    expected_intermediate_rows: tuple[tuple[object, ...], ...]
    expected_stdout_fragments: tuple[str, ...] = field(default_factory=tuple)
    expected_return_code: int = 0


@dataclass(frozen=True)
class ChainedLoaderPruningE2ETestCase:
    description: str
    command: tuple[str, ...]
    expected_raw_events_exists: bool
    expected_intermediate_exists: bool
    expected_return_code: int = 0


@dataclass(frozen=True)
class ChainedLoaderFailureE2ETestCase:
    description: str
    command: tuple[str, ...]
    expected_ok_rows: tuple[tuple[object, ...], ...]
    expected_error_fragment: str
    expected_skip_fragment: str
    expected_return_code: int = 1


@dataclass(frozen=True)
class IntermediateLoaderStrategyE2ETestCase:
    description: str
    loader_py: str
    expected_intermediate_rows: tuple[tuple[object, ...], ...]
    expected_terminal_rows: tuple[tuple[object, ...], ...]
    command: tuple[str, ...] = ("--no-color", "load", "--select", "+raw_events")
    expected_return_code: int = 0


@dataclass(frozen=True)
class SourceOnlyIngressDependencyE2ETestCase:
    description: str
    setup_command: tuple[str, ...] | None
    command: tuple[str, ...]
    expected_return_code: int
    expected_stdout_fragments: tuple[str, ...] = field(default_factory=tuple)
    expected_error_fragment: str | None = None
    expected_intermediate_rows: tuple[tuple[object, ...], ...] = ()
    expected_terminal_rows: tuple[tuple[object, ...], ...] = ()
    expected_marker_exists: bool = False


@dataclass(frozen=True)
class SourceOnlyComplexIngressE2ETestCase:
    description: str
    command: tuple[str, ...]
    expected_return_code: int
    dependency_dir: str = ""
    decorator_import: str = ""
    decorator_name: str = ""
    expected_error_fragment: str | None = None
    expected_stdout_fragments: tuple[str, ...] = field(default_factory=tuple)
    setup_commands: tuple[tuple[str, ...], ...] = field(default_factory=tuple)


@dataclass(frozen=True)
class LoaderWaffleShopE2ETestCase:
    description: str
    command: tuple[str, ...]
    run_count: int
    expected_order_count: int
    expected_customer_revenue_rows: tuple[tuple[object, ...], ...]
    expected_intermediate_counts: tuple[tuple[str, int], ...]
    expected_return_code: int = 0


@dataclass(frozen=True)
class SourceLoaderErrorE2ETestCase:
    description: str
    repo_files: dict[str, str]
    command: tuple[str, ...]
    expected_error_fragment: str
    expected_return_code: int = 1


@dataclass(frozen=True)
class DltSqlDatabaseE2ETestCase:
    description: str
    expected_loaded_rows: tuple[tuple[object, ...], ...]
    expected_model_rows: tuple[tuple[object, ...], ...]


@dataclass(frozen=True)
class DltLocalSourceE2ETestCase:
    description: str
    expected_loaded_rows: tuple[tuple[object, ...], ...]
    expected_model_rows: tuple[tuple[object, ...], ...]


@dataclass(frozen=True)
class LoaderRelationGuardE2ETestCase:
    description: str
    enforce_explicit: bool
    expected_warnings: tuple[str, ...]
    expected_final_line_prefix: str


@dataclass(frozen=True)
class IntermediateLoaderDefaultE2ETestCase:
    description: str
    command: tuple[str, ...]
    expected_intermediate_rows: tuple[tuple[object, ...], ...]
    expected_terminal_rows: tuple[tuple[object, ...], ...]
