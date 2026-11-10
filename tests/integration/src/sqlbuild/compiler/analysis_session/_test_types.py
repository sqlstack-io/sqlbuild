from __future__ import annotations

from dataclasses import dataclass
from typing import TYPE_CHECKING

from sqlbuild._native import ColumnLineageMode
from sqlbuild.adapter.contract.models import ExpressionInferenceProfile

if TYPE_CHECKING:
    from sqlbuild._native import QueryColumnsMode


@dataclass(frozen=True)
class GeneratedAnalysisTestCase:
    """Seeded generated projects whose uncached, cached and re-proven native analyses agree."""

    description: str
    seed: int
    count: int
    model_count: int
    inference_profiles: tuple[ExpressionInferenceProfile, ...]
    extra_files: dict[str, str]
    lineage_mode: ColumnLineageMode
    expected_minimum_native: int
    expected_minimum_expression_shapes: int
    expected_minimum_pivot_proofs: int
    expected_minimum_proven_pivots: int
    expected_minimum_native_enrichments: int


@dataclass(frozen=True)
class CyclicAnalysisTestCase:
    """Models that `ref` each other, which the native session analyses in one unordered wave."""

    description: str
    files: dict[str, str]
    dialects: tuple[str, ...]
    lineage_modes: tuple[ColumnLineageMode, ...]
    expected_analysed: int


@dataclass(frozen=True)
class SourceTypesTestCase:
    """A model reading a source with declared types, with or without the source's expression."""

    description: str
    files: dict[str, str]
    lineage_mode: ColumnLineageMode
    expected_types: tuple[tuple[str, str | None], ...]
    dialect: str = "duckdb"


@dataclass(frozen=True)
class AdapterWaveTestCase:
    """A shipped adapter's own profile, whose session must analyse each model natively."""

    description: str
    profile: ExpressionInferenceProfile
    expected_shared: int


@dataclass(frozen=True)
class CyclicCompileTestCase:
    """A project whose models `ref` each other, compiled through the CLI."""

    description: str
    files: dict[str, str]
    expected_exit_code: int
    expected_compiled: tuple[str, ...]


@dataclass(frozen=True)
class StandalonePivotProofTestCase:
    """A pivot model left out of model analysis, whose proof assembly takes natively."""

    description: str
    analysed_models: frozenset[str]
    expected_native_proofs: int
    expected_session_proofs: int
    expected_proven_by_model: dict[str, bool | None]


@dataclass(frozen=True)
class SharedAnalysisTestCase:
    """Models with equal queries, some inexact, and how many analyses the session shares."""

    description: str
    regions: tuple[str, ...]
    inexact_regions: tuple[str, ...]
    dialects: tuple[str | None, ...]
    expected_analysed: int
    expected_shared: int
    expected_reanalysed: int
    expected_unshared: int
    expected_column_values: int


@dataclass(frozen=True)
class CteFactRecoveryParityTestCase:
    """Seeded CTE queries whose native and Python CTE fact recoveries must agree."""

    description: str
    seed: int
    count: int
    expected_minimum_compared: int
    expected_minimum_recovered: dict[str, int]


@dataclass(frozen=True)
class AdapterRuleCallbackTestCase:
    """An adapter's own nullability rule, what it raises if called, and the native warning."""

    description: str
    seed: int
    model_count: int
    raised: type[Exception] | None
    expected_warning: str


@dataclass(frozen=True)
class SharedQueryOutputTestCase:
    """Models with equal query shapes over different inputs, and each model's own outputs."""

    description: str
    models: dict[str, str]
    expected_lineage: tuple[tuple[str, str, str], ...] = ()
    expected_findings: tuple[tuple[str, str], ...] = ()


@dataclass(frozen=True)
class FallbackColumnsTestCase:
    """A model native analysis cannot project, and the columns its parsed tree still gives."""

    description: str
    query_sql: str
    expected_columns: tuple[tuple[str, str | None, str], ...]


@dataclass(frozen=True)
class TableFunctionColumnsTestCase:
    """A table function's declared return columns against its body's output columns."""

    description: str
    returns: str
    body_sql: str
    expected_error: str | None


@dataclass(frozen=True)
class ProjectionFactsTestCase:
    """Fixture queries whose projection facts Python recorded in a golden."""

    description: str
    queries: tuple[tuple[str, str], ...]
    expected_differences: list[tuple[int, str, object, object]]


@dataclass(frozen=True)
class NormalizationRejectionTestCase:
    """Queries outside the session whose analysis SQL Python's normalization rejects."""

    description: str
    queries: tuple[tuple[str, QueryColumnsMode], ...]
    dialect: str
    expected_message: str
