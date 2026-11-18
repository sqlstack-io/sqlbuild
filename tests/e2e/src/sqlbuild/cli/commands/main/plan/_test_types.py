from dataclasses import dataclass, field


@dataclass(frozen=True)
class RemovedInterfaceTestCase:
    description: str
    command: tuple[str, ...]
    expected_error: str
    expected_exit_code: int = 2


@dataclass(frozen=True)
class RemovedConfigTestCase:
    description: str
    content: str
    expected_key: str
    filename: str = "sqlbuild_project.toml"
    prefix: str = 'name = "orders"\nadapter = "duckdb"\n'
    expected_exit_code: int = 1


@dataclass(frozen=True)
class RemovedHookTestCase:
    description: str
    expected_error: str
    expected_exit_code: int = 1


@dataclass(frozen=True)
class TerminalPlanProgressE2ETestCase:
    description: str
    runs: int
    columns: int
    expected_screen_fragments: tuple[str, ...]
    expected_raw_fragments: tuple[str, ...]


@dataclass(frozen=True)
class DiamondPlanE2ETestCase:
    description: str
    expected_fragments: tuple[str, ...]
    expected_max_lines_per_model: int
    unexpected_fragments: tuple[str, ...] = ()


@dataclass(frozen=True)
class DiamondPlanJsonE2ETestCase:
    description: str
    expected_reasons: dict[str, str]


@dataclass(frozen=True)
class VerboseInspectionPlanE2ETestCase:
    description: str
    command: tuple[str, ...]
    expected_inspection_output: bool
    expected_json_stdout: bool


@dataclass(frozen=True)
class MissingSourceTableE2ETestCase:
    description: str
    expected_output_fragments: tuple[str, ...] = ()
    expected_recorded_source_names: tuple[str, ...] = ()
    expected_statuses: dict[str, str] = field(default_factory=dict)


@dataclass(frozen=True)
class PlanExitCodeE2ETestCase:
    description: str
    added_files: dict[str, str]
    plan_args: tuple[str, ...]
    expected_exit_code: int
    expected_output_fragments: tuple[str, ...]


@dataclass(frozen=True)
class PreviewPerformanceTestCase:
    """A synced 10K-model warehouse and the budgets a warm `--changed` preview of it meets."""

    description: str
    built_share: float
    expected_unbuilt: tuple[int, int]
    expected_definition_changes: tuple[int, int]
    #: The warm preview's wall time, from queueing the request to the stored plan.
    expected_max_warm_seconds: float
    #: The warm preview's share of the location's publish, a linear 10K compile on one machine.
    expected_max_warm_to_publish: float
    expected_max_check_seconds: float
    #: A live plan of the same warehouse, compile included, as a multiple of the warm preview.
    expected_max_live_to_warm: float
    #: Selected models analysed again against inspected source columns in the live plan.
    expected_source_rebinding: tuple[int, int]
    #: Every statement the live plan sends the warehouse, cursor batches included.
    expected_max_warehouse_statements: int


@dataclass(frozen=True)
class PlanTimingsE2ETestCase:
    description: str
    plan_args: tuple[str, ...]
    expected_first_step: str
    expected_last_step: str
    expected_steps: tuple[str, ...] = ()


@dataclass(frozen=True)
class PlanWithoutTimingsE2ETestCase:
    description: str
    plan_args: tuple[str, ...]
    expected_table_printed: bool


@dataclass(frozen=True)
class PlanFromE2ETestCase:
    """Where a DuckDB plan is asked to read warehouse state, and what it says on stderr."""

    description: str
    plan_args: tuple[str, ...]
    expected_stderr: str
    unexpected_stderr: str
