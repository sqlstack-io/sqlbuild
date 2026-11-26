from dataclasses import dataclass, field


@dataclass(frozen=True)
class BigQueryCliTestCase:
    description: str
    command: tuple[str, ...]
    expected_stdout_fragments: tuple[str, ...] = field(default_factory=tuple)
    expected_return_code: int = 0


@dataclass(frozen=True)
class BigQueryDbtProfileE2ETestCase:
    description: str
    schema_prefix: str
    expected_toml_fragments: tuple[str, ...]


@dataclass(frozen=True)
class BigQueryNodeResultE2ETestCase:
    description: str
    expected_rows: tuple[tuple[object, ...], ...]
    expected_return_code: int = 0


@dataclass(frozen=True)
class BigQueryBuildE2ETestCase:
    description: str
    expected_table_name: str
    expected_row_count: int
    expected_fact_order_rows: tuple[tuple[object, ...], ...] = field(default_factory=tuple)
    expected_udf_rows: tuple[tuple[object, ...], ...] = field(default_factory=tuple)
    expected_python_udf_rows: tuple[tuple[object, ...], ...] = field(default_factory=tuple)
    expected_daily_revenue_rows: tuple[tuple[object, ...], ...] = field(default_factory=tuple)
    command: tuple[str, ...] = field(default_factory=tuple)
    expected_stdout_fragments: tuple[str, ...] = field(default_factory=tuple)
    expected_return_code: int = 0


@dataclass(frozen=True)
class BigQuerySourceLoaderStrategiesE2ETestCase:
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
class BigQuerySourceLoaderSchemaEvolutionE2ETestCase:
    description: str
    command: tuple[str, ...]
    expected_rows: tuple[tuple[object, ...], ...]
    expected_return_code: int = 0


@dataclass(frozen=True)
class BigQueryIntermediateDagStrategyE2ETestCase:
    description: str
    loader_py: str
    expected_intermediate_rows: tuple[tuple[object, ...], ...]
    expected_terminal_rows: tuple[tuple[object, ...], ...]
    command: tuple[str, ...] = ("--no-color", "load", "--select", "+raw_events")
    expected_return_code: int = 0


@dataclass(frozen=True)
class BigQueryDiffE2ETestCase:
    description: str
    mutation_sql: tuple[str, ...]
    command: tuple[str, ...]
    expected_stdout_fragments: tuple[str, ...] = field(default_factory=tuple)
    expected_return_code: int = 0


@dataclass(frozen=True)
class BigQueryCloneE2ETestCase:
    description: str
    default_command: tuple[str, ...]
    hard_copy_command: tuple[str, ...]
    expected_default_stdout_fragments: tuple[str, ...]
    expected_hard_copy_stdout_fragments: tuple[str, ...]
    expected_rows: tuple[tuple[object, ...], ...]


@dataclass(frozen=True)
class BigQueryModelBuildE2ETestCase:
    description: str
    model_name: str
    expected_sql_fragment: str
    expected_return_code: int = 0


@dataclass(frozen=True)
class BigQueryErrorE2ETestCase:
    description: str
    command: tuple[str, ...]
    expected_error_fragment: str
    expected_return_code: int = 1


@dataclass(frozen=True)
class BigQueryScenarioRemoteE2ETestCase:
    description: str
    expected_stdout_fragments: tuple[str, ...]
    expected_retained_suffix_counts: dict[str, int]
    expected_row_counts_by_suffix: dict[str, int]


@dataclass(frozen=True)
class BigQuerySnapshotE2ETestCase:
    description: str
    expected_current_rows_after_initial_build: tuple[tuple[object, ...], ...]
    expected_current_rows_after_recovery: tuple[tuple[object, ...], ...]
    expected_historical_timestamp_rows: tuple[tuple[object, ...], ...]
    expected_historical_check_rows: tuple[tuple[object, ...], ...]
    expected_failure_fragments: tuple[str, ...]


@dataclass(frozen=True)
class BigQuerySnapshotApplyE2ETestCase:
    description: str
    expected_current_check_rows: tuple[tuple[object, ...], ...]
    expected_current_delete_rows: tuple[tuple[object, ...], ...]
    expected_historical_timestamp_rows: tuple[tuple[object, ...], ...]
    expected_historical_check_rows: tuple[tuple[object, ...], ...]


@dataclass(frozen=True)
class BigQuerySourceDeferralE2ETestCase:
    description: str
    expected_model_rows: tuple[tuple[object, ...], ...]
    expected_loader_rows: tuple[tuple[object, ...], ...]
    command: tuple[str, ...] = ("--no-color", "build", "--select", "stg_orders")
    expected_return_code: int = 0
