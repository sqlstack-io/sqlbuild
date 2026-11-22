"""Test case types for SQL test pipeline helpers."""

from __future__ import annotations

from dataclasses import dataclass

from sqlbuild._native import PlanOutput


@dataclass(frozen=True)
class SqlTestFunctionPreflightTestCase:
    """One SQL test function preflight scenario."""

    description: str
    expected_outcome: str
    expected_error_fragment: str


@dataclass(frozen=True)
class SqlTestOperationLifecycleTestCase:
    description: str
    expected_resource_id: str
    expected_operation_name: str


@dataclass(frozen=True)
class SqlTestConcurrencyTestCase:
    description: str
    test_count: int
    max_concurrency: int
    expected_connection_count: int


@dataclass(frozen=True)
class AuditPipelineLifecycleTestCase:
    description: str
    expected_event_type: str
    expected_order: tuple[str, ...]


@dataclass(frozen=True)
class AuditResourceIdentityTestCase:
    description: str
    expected_first_id: str
    expected_second_id: str


@dataclass(frozen=True)
class AuditLogicalIdentityTestCase:
    description: str
    expected_id: str


@dataclass(frozen=True)
class AuditConcurrencyTestCase:
    description: str
    expected_count: int = 0
    expected_names: tuple[str, ...] = ()
    expected_error: str = ""


@dataclass(frozen=True)
class AuditStopErrorTestCase:
    description: str
    error: BaseException
    expected_started_names: tuple[str, ...]
    expected_connection_count: int


@dataclass(frozen=True)
class AuditErrorFormattingTestCase:
    description: str
    error: Exception
    expected_message: str


@dataclass(frozen=True)
class AuditPhysicalCallbackTestCase:
    description: str
    max_concurrency: int
    expected_started_names: tuple[str, ...]
    expected_possible_started_names: tuple[str, ...]
    expected_connection_count: int
    expected_pass_count: int


@dataclass(frozen=True)
class ScenarioFailureHelpTestCase:
    """One scenario failure help resolution case."""

    description: str
    error: Exception
    expected_help: str | None


@dataclass(frozen=True)
class ScenarioConnectionLifecycleTestCase:
    """One warehouse scenario pipeline target-connection lifecycle case."""

    description: str
    expected_event_types: tuple[str, ...]
    expected_operation_names: tuple[str, ...]
    expected_connection_events: tuple[str, ...]


@dataclass(frozen=True)
class ScenarioTestPipelineTestCase:
    """One scenario test pipeline orchestration case."""

    description: str
    scenario_names: tuple[str, ...]
    planning_failure_name: str
    expected_statuses: tuple[str, ...]
    expected_started_names: tuple[str, ...]
    expected_completed_names: tuple[str, ...]
    expected_completed_plan_names: tuple[str | None, ...]
    expected_connection_events: tuple[str, ...]
    expected_error_fragment: str


@dataclass(frozen=True)
class ScenarioLocalPipelineTestCase:
    """One local scenario load-only pipeline case."""

    description: str
    snapshot_state: str
    strict: bool
    load_error_message: str | None
    expected_local_status: str
    expected_status: str
    expected_retained: bool
    expected_duckdb_exists: bool
    expected_error_code: str | None


@dataclass(frozen=True)
class ScenarioPlanningLifecycleTestCase:
    description: str
    expected_resource_id: str
    expected_run_id: str
    expected_terminal: str


@dataclass(frozen=True)
class SeedPipelineQueueTestCase:
    """Seeds behind a slow one: which load before it finishes."""

    description: str
    seed_names: tuple[str, ...]
    slow_seed: str
    max_concurrency: int
    expected_load_order: tuple[str, ...]


@dataclass(frozen=True)
class SeedFingerprintSetupTestCase:
    """Concurrent seeds on an empty schema: how often the fingerprint table is set up."""

    description: str
    seed_names: tuple[str, ...]
    max_concurrency: int
    expected_table_setups: int
    expected_fingerprint_rows: int


@dataclass(frozen=True)
class SeedPipelineConcurrencyTestCase:
    """One seed pipeline concurrency orchestration case."""

    description: str
    seed_names: tuple[str, ...]
    max_concurrency: int
    expected_connection_count: int
    expected_seed_order: tuple[str, ...]
    expected_json_asset_order: tuple[str, ...]


@dataclass(frozen=True)
class SeedPipelineLifecycleTestCase:
    """One standalone seed attempt lifecycle expectation."""

    description: str
    seed_names: tuple[str, ...]
    expected_terminal_types: tuple[str, ...]
    expected_resource_ids: frozenset[str]


@dataclass(frozen=True)
class WorkerConnectionTestCase:
    description: str
    connection_count: int
    expected_connection_count: int
    expected_close_attempts: int = 0


@dataclass(frozen=True)
class ConnectionPreparationTimingTestCase:
    description: str
    clock_values: tuple[float, ...]
    expected_connection_seconds: float
    expected_schema_seconds: float
    expected_total_seconds: float


@dataclass(frozen=True)
class ScenarioWorkerExitTestCase:
    """Scenario workers left inside native code when the interpreter exits, and the exit code."""

    description: str
    worker_count: int
    expected_returncode: int


@dataclass(frozen=True)
class TemporaryTestFunctionTestCase:
    description: str
    body_sql: str
    test_count: int
    max_concurrency: int
    expected_outcomes: tuple[str, ...]
    expected_error_fragments: tuple[str, ...]


@dataclass(frozen=True)
class RefusedTemporaryCopyTestCase:
    description: str
    plan: PlanOutput
    expected_function: str


@dataclass(frozen=True)
class TemporaryCopyPlacementTestCase:
    description: str
    session_schema: tuple[str, str] | None
    expected_target: str


@dataclass(frozen=True)
class FunctionCallRewriteTestCase:
    description: str
    sql: str
    expected_sql: str
