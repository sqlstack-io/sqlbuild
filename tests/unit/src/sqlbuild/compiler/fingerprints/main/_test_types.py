from dataclasses import dataclass


@dataclass(frozen=True)
class ReadLatestFingerprintsTestCase:
    description: str
    rows: list[tuple[object, ...]]
    expected_model_name: str
    expected_version_hash: str
    expected_query_sql: str
    expected_metadata_json: str


@dataclass(frozen=True)
class ReadLatestFingerprintsErrorTestCase:
    description: str
    read_error: Exception
    expected_message_fragment: str


@dataclass(frozen=True)
class ReadLatestFingerprintsRendererTestCase:
    description: str
    expected_executed_sql: str


@dataclass(frozen=True)
class ReadLatestFingerprintsFilterTestCase:
    description: str
    node_names: tuple[str, ...]
    expected_sql_fragment: str


@dataclass(frozen=True)
class WriteFingerprintIndexTestCase:
    description: str
    expected_index_sql: str
    expected_insert_prefix: str


@dataclass(frozen=True)
class WriteFingerprintRetryTestCase:
    description: str
    failing_create_attempts: int
    error_message: str
    expected_create_attempts: int
    expected_insert_count: int
    expected_sleep_count: int


@dataclass(frozen=True)
class WriteFingerprintRetryExhaustionTestCase:
    description: str
    error_message: str
    expected_create_attempts: int
    expected_insert_count: int
    expected_error_fragment: str


@dataclass(frozen=True)
class WriteFingerprintBatchTestCase:
    description: str
    fingerprint_count: int
    definition_size: int
    expected_rows_per_insert: tuple[int, ...]
    expected_progress: tuple[tuple[int, int], ...]


@dataclass(frozen=True)
class WriteFingerprintPartialFailureTestCase:
    description: str
    fingerprint_count: int
    expected_execute_count: int
    expected_progress: tuple[tuple[int, int], ...]


@dataclass(frozen=True)
class NormalizeQuerySqlTestCase:
    description: str
    query_sql: str
    expected_normalized: str


@dataclass(frozen=True)
class ComputeQueryHashTestCase:
    description: str
    dialect: str | None
    query_sql: str
    expected_hash: str


@dataclass(frozen=True)
class ComputeQueryHashStabilityTestCase:
    description: str
    dialect: str | None
    query_a: str
    query_b: str
    expected_same_hash: bool


@dataclass(frozen=True)
class QueryFingerprintFailureTestCase:
    description: str
    dialect: str
    query_sql: str
    expected_message: str


@dataclass(frozen=True)
class QueryFingerprintTimeLimitTestCase:
    description: str
    dialect: str
    query_sql: str
    expected_max_seconds: float
