from dataclasses import dataclass, field


@dataclass(frozen=True)
class QueryCliTestCase:
    description: str
    command: tuple[str, ...]
    expected_stdout_fragment: str
    expected_returncode: int = 0


@dataclass(frozen=True)
class QueryFileCliTestCase:
    description: str
    query_file_path: str
    query_sql: str
    expected_stdout_fragment: str


@dataclass(frozen=True)
class QueryFileErrorCliTestCase:
    description: str
    command: tuple[str, ...]
    expected_stderr_fragment: str
    repo_files: dict[str, str] = field(default_factory=dict)
    binary_files: dict[str, bytes] = field(default_factory=dict)


@dataclass(frozen=True)
class QueryExecutionErrorCliTestCase:
    description: str
    command: tuple[str, ...]
    expected_stderr_fragment: str
    expected_returncode: int = 2


@dataclass(frozen=True)
class QueryUnrelatedProjectFaultTestCase:
    description: str
    repo_files: dict[str, str]
    expected_rows: tuple[dict[str, int], ...] = ({"order_id": 7},)


@dataclass(frozen=True)
class QueryProjectDiscoveryTestCase:
    description: str
    command: tuple[str, ...]
    expected_exit_code: int
    expected_project_discoveries: int


@dataclass(frozen=True)
class QueryTargetConnectionTestCase:
    description: str
    command: tuple[str, ...]
    expected_stdout_fragment: str


@dataclass(frozen=True)
class QueryConnectionConfigErrorTestCase:
    description: str
    project_toml: str
    expected_stderr_fragment: str
    expected_returncode: int = 1


@dataclass(frozen=True)
class QueryProgressTestCase:
    description: str
    sql: str
    use_pty: bool
    expected_fragments: tuple[str, ...]
    unexpected_fragments: tuple[str, ...]
