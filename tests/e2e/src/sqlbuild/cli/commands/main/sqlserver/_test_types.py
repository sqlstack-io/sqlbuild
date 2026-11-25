from dataclasses import dataclass, field


@dataclass(frozen=True)
class SqlServerBuildE2ETestCase:
    description: str
    expected_row_count: int
    expected_table_name: str = "fact_orders"
    command: tuple[str, ...] = field(default_factory=tuple)
    expected_stdout_fragments: tuple[str, ...] = field(default_factory=tuple)
    expected_return_code: int = 0


@dataclass(frozen=True)
class SqlServerCloneE2ETestCase:
    description: str
    expected_stdout_fragments: tuple[str, ...]
    expected_rows: tuple[tuple[object, ...], ...]
    expected_return_code: int = 0


@dataclass(frozen=True)
class SqlServerDiffE2ETestCase:
    description: str
    command: tuple[str, ...]
    mutation_sql: tuple[str, ...]
    expected_stdout_fragments: tuple[str, ...]
    expected_return_code: int
    expected_absent_stdout_fragments: tuple[str, ...] = ()


@dataclass(frozen=True)
class SqlServerDbtProfileE2ETestCase:
    description: str
    schema_prefix: str
    expected_toml_fragments: tuple[str, ...]


@dataclass(frozen=True)
class SqlServerNodeResultE2ETestCase:
    description: str
    expected_rows: tuple[tuple[object, ...], ...]
    expected_return_code: int = 0


@dataclass(frozen=True)
class SqlServerSourceLoaderE2ETestCase:
    description: str
    expected_rows: tuple[tuple[str, ...], ...]
    expected_return_code: int = 0


@dataclass(frozen=True)
class SqlServerSourceDeferralE2ETestCase:
    description: str
    expected_model_rows: tuple[tuple[str, ...], ...]
    expected_loader_rows: tuple[tuple[str, ...], ...]
    expected_return_code: int = 0


@dataclass(frozen=True)
class SqlServerLoaderWaffleShopE2ETestCase:
    description: str
    expected_rows: tuple[tuple[str, ...], ...]
    expected_event_count: int
    expected_return_code: int = 0


@dataclass(frozen=True)
class SqlServerSourceLoaderStrategiesE2ETestCase:
    description: str
    expected_countries: tuple[tuple[str, ...], ...]
    expected_webhook_event_counts: tuple[tuple[str, ...], ...]
    expected_order_events: tuple[tuple[str, ...], ...]
    expected_customers: tuple[tuple[str, ...], ...]
    expected_loader_status: tuple[tuple[str, ...], ...]
    expected_return_code: int = 0


@dataclass(frozen=True)
class SqlServerIntermediateDagStrategyE2ETestCase:
    description: str
    loader_py: str
    expected_intermediate_rows: tuple[tuple[str, ...], ...]
    expected_terminal_rows: tuple[tuple[str, ...], ...]
    command: tuple[str, ...] = ("--no-color", "load", "--select", "+raw_events")
    expected_return_code: int = 0


@dataclass(frozen=True)
class SqlServerSourceLoaderDagE2ETestCase:
    description: str
    command: tuple[str, ...]
    expected_rows: tuple[tuple[str, ...], ...]
    expected_return_code: int = 0


@dataclass(frozen=True)
class SqlServerSnapshotE2ETestCase:
    description: str
    expected_current_rows_after_initial_build: tuple[tuple[object, ...], ...]
    expected_current_rows_after_recovery: tuple[tuple[object, ...], ...]
    expected_historical_timestamp_rows: tuple[tuple[object, ...], ...]
    expected_historical_check_rows: tuple[tuple[object, ...], ...]
    expected_failure_fragments: tuple[str, ...]


@dataclass(frozen=True)
class SqlServerSnapshotApplyE2ETestCase:
    description: str
    expected_current_check_rows: tuple[tuple[object, ...], ...]
    expected_current_delete_rows: tuple[tuple[object, ...], ...]
    expected_historical_timestamp_rows: tuple[tuple[object, ...], ...]
    expected_historical_check_rows: tuple[tuple[object, ...], ...]
