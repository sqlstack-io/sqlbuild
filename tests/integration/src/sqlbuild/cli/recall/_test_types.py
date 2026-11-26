import signal
from collections.abc import Callable
from dataclasses import dataclass
from pathlib import Path

from tests.integration.src.sqlbuild.cli.recall.helpers import PYTHON_CLI


@dataclass(frozen=True)
class RecallOutputTestCase:
    description: str
    args: tuple[str, ...]
    expected_stream: str


@dataclass(frozen=True)
class CombinedOutputTestCase:
    description: str
    expected_order: tuple[bytes, ...]


@dataclass(frozen=True)
class RecordedFactsTestCase:
    description: str
    sql: str
    expected_rows: list[list[object]]


@dataclass(frozen=True)
class JsonQueryTestCase:
    description: str
    args: tuple[str, ...]
    expected_row_count: int
    expected_truncated: bool
    expected_columns: list[str]


@dataclass(frozen=True)
class TextOutputTestCase:
    description: str
    args: tuple[str, ...]
    expected_fragments: tuple[str, ...]


@dataclass(frozen=True)
class RecallRejectionTestCase:
    description: str
    project_suffix: str
    args: tuple[str, ...]
    expected_code: str


@dataclass(frozen=True)
class DisabledRecallTestCase:
    description: str
    project_suffix: str
    expected_store_exists: bool


@dataclass(frozen=True)
class ReadOnlyStoreTestCase:
    description: str
    expected_fragments: tuple[str, ...]
    expected_repeat_notice: bool


@dataclass(frozen=True)
class AbandonedRunTestCase:
    description: str
    heartbeat_seconds: str
    expected_status: str
    expected_output_complete: int


@dataclass(frozen=True)
class ConcurrentBuildsTestCase:
    description: str
    targets: tuple[str, ...]
    expected_status: str


@dataclass(frozen=True)
class RepeatedBuildTestCase:
    description: str
    builds: int
    sql: str
    expected_rows: list[list[object]]


@dataclass(frozen=True)
class RecordedCommandTestCase:
    description: str
    args: tuple[str, ...]
    expected_recorded: list[list[object]]


@dataclass(frozen=True)
class HeldLockTestCase:
    description: str
    shutdown_seconds: float
    builds_before_lock: int
    expected_exit_code: int
    expected_notice: str
    expected_failed_records: int
    expected_recorded_builds: int
    #: Far beyond the budget; only a build that waits for the lock itself reaches it.
    hang_guard_seconds: float


@dataclass(frozen=True)
class ConcurrentWritersTestCase:
    description: str
    lines_per_stream: int
    expected_order: list[int]


@dataclass(frozen=True)
class CorruptStoreTestCase:
    description: str
    expected_notice_prefix: str
    expected_recorded: list[list[object]]


@dataclass(frozen=True)
class FailingStderrTestCase:
    description: str
    expected_exit_code: int


@dataclass(frozen=True)
class CorruptStoreRaceTestCase:
    description: str
    targets: tuple[str, ...]
    expected_exit_codes: list[int]
    expected_moved: int
    expected_recorded: list[list[object]]


@dataclass(frozen=True)
class CaptureLimitTestCase:
    description: str
    extra_toml: str
    stdout_bytes: int
    expected_kept: tuple[int, int]
    expected_gap: list[object]


@dataclass(frozen=True)
class TruncationNoteTestCase:
    description: str
    stdout_bytes: int
    expected_kept: int


@dataclass(frozen=True)
class InvalidOutputLimitTestCase:
    description: str
    value: str
    expected_message: bytes


@dataclass(frozen=True)
class UndecodableArgumentTestCase:
    description: str
    selector: str
    expected_error: bytes
    expected_recorded: list[list[object]]


@dataclass(frozen=True)
class ReplacementLifecycleTestCase:
    description: str
    dropped_prefix: str
    expected_filter_active: int


@dataclass(frozen=True)
class ReplacementOutputTestCase:
    description: str
    replaced: tuple[bytes, bytes]
    expected_complete: int


@dataclass(frozen=True)
class FailingReplacementTestCase:
    description: str
    expected_failed_streams: list[list[object]]
    expected_notice: bytes


@dataclass(frozen=True)
class ReplacementLaunchRecordTestCase:
    description: str
    sinks: dict[str, str]
    expected_records: int


@dataclass(frozen=True)
class SlowReplacementTestCase:
    description: str
    shutdown_seconds: float
    sink_sleep_seconds: str
    expected_max_seconds: float


@dataclass(frozen=True)
class InterruptedSinkTestCase:
    description: str
    shutdown_seconds: float
    sink_sleep_seconds: str
    expected_exit_code: int


@dataclass(frozen=True)
class ThirdPartyLogTestCase:
    description: str
    build_arguments: tuple[str, ...]
    expected_present: tuple[str, ...]
    expected_absent: tuple[str, ...]
    expected_traceback_count: int


@dataclass(frozen=True)
class RecallConnectionErrorTestCase:
    description: str
    recall_toml: str
    args: tuple[str, ...]
    expected_message: str


@dataclass(frozen=True)
class ReadOnlyReaderTestCase:
    description: str
    expected_shared_invocations: list[list[object]]


@dataclass(frozen=True)
class BehindStoreTestCase:
    description: str
    expected_ddl_fragments: tuple[str, ...]
    expected_up_to_date_note: str


@dataclass(frozen=True)
class JsonErrorTestCase:
    description: str
    args: tuple[str, ...]
    expected_code: str


@dataclass(frozen=True)
class RemovedFormatTestCase:
    description: str
    args: tuple[str, ...]
    expected_message: str


@dataclass(frozen=True)
class QueryLayoutTestCase:
    description: str
    args: tuple[str, ...]
    expected_fragments: tuple[str, ...]
    unexpected_fragments: tuple[str, ...]


@dataclass(frozen=True)
class FailureCauseTestCase:
    description: str
    args: tuple[str, ...]
    expected_failure: list[object]
    expected_message_prefix: str


@dataclass(frozen=True)
class GitRevisionTestCase:
    description: str
    prepare: Callable[[Path], tuple[Path, str]]
    environment: dict[str, str]
    expected_dirty: object


@dataclass(frozen=True)
class LeanRecallTestCase:
    description: str
    args: tuple[str, ...]
    expected_sink_log: bool


@dataclass(frozen=True)
class StatementStartTestCase:
    description: str
    sql: str
    expected_rows: list[list[object]]


@dataclass(frozen=True)
class StoreBeingCreatedTestCase:
    description: str
    args: tuple[str, ...]
    expected_error: str


@dataclass(frozen=True)
class SplitCredentialsTestCase:
    description: str
    writer_environment: dict[str, str]
    reader_environment: dict[str, str]
    expected_invocations: list[list[object]]


@dataclass(frozen=True)
class LayeredConnectionsTestCase:
    description: str
    project_toml: str
    local_toml: str
    expected_invocations: list[list[object]]


@dataclass(frozen=True)
class RecallUsageErrorTestCase:
    description: str
    args: tuple[str, ...]
    expected_message: str


@dataclass(frozen=True)
class RecallDiagnosticsTestCase:
    description: str
    global_args: tuple[str, ...]
    expected_debug_log: bool


@dataclass(frozen=True)
class UnsetWriterVariableTestCase:
    description: str
    expected_exit_code: int


@dataclass(frozen=True)
class DocumentedQueryTestCase:
    description: str
    question: int
    columns: tuple[str, ...]
    expected_rows: list[list[object]]


@dataclass(frozen=True)
class InvocationOriginTestCase:
    description: str
    args: tuple[str, ...]
    environment: dict[str, str]
    expected_rows: list[list[object]]


@dataclass(frozen=True)
class IntegrationColumnsTestCase:
    description: str
    stored_context: str
    expected_columns: list[object]


@dataclass(frozen=True)
class InvocationPrefixTestCase:
    description: str
    prefix_length: int
    expected_exit_code: int
    expected_error_code: str | None


@dataclass(frozen=True)
class RecordedSelectionTestCase:
    description: str
    selector_file: str
    args: tuple[str, ...]
    expected_selection: list[object]


@dataclass(frozen=True)
class TerminatedCommandTestCase:
    description: str
    orders_model: str
    environment: dict[str, str]
    wait_sql: str
    expected_output: bytes
    unfinished_output: bytes
    interrupts: signal.Handlers = signal.SIG_DFL
    program: Path = PYTHON_CLI


@dataclass(frozen=True)
class FailedStatementSqlTestCase:
    description: str
    model_sql: str
    expected_sql_fragment: str


@dataclass(frozen=True)
class AuditEvidenceTestCase:
    description: str
    recall_toml: str
    severity: str
    expected_exit_code: int
    expected_evidence: list[object]
    expected_artifacts: list[list[object]]
    expected_warehouse_evidence: list[tuple[object, ...]]


@dataclass(frozen=True)
class CostAttributionTestCase:
    description: str
    collected: str
    expected_cost_status: str


@dataclass(frozen=True)
class ClosingTerminationTestCase:
    description: str
    shutdown_timeout: str
    expected_exit_code: int


@dataclass(frozen=True)
class LocalStatementsTestCase:
    description: str
    args: tuple[str, ...]
    sql: str
    expected_rows: list[list[object]]
