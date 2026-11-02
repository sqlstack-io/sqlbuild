from __future__ import annotations

import re
from collections.abc import Callable
from dataclasses import dataclass

from tests.integration.src.sqlbuild.executor.build.snowflake_metadata.helpers import (
    SimulatedSnowflakeWarehouse,
)


@dataclass(frozen=True)
class BuildMetadataBudgetTestCase:
    """Two consecutive offline Snowflake builds and the column reads each may issue."""

    description: str
    max_concurrency: int
    expected_first_column_reads: dict[str, int]
    expected_second_column_reads: dict[str, int]


@dataclass(frozen=True)
class SchemaEvolutionTestCase:
    """A merge model whose delta gains a column between builds."""

    description: str
    added_column: str
    expected_target_reads: int
    expected_merge_fragment: str


@dataclass(frozen=True)
class StateTableRetentionTestCase:
    """An offline account edition and the time travel its state tables are created with."""

    description: str
    max_retention_days: int
    expected_first_retention_days: str
    expected_later_retention_days: frozenset[str]


@dataclass(frozen=True)
class SourceBindingCaseTestCase:
    """A select list over an existing source and the binding error planning should report."""

    description: str
    select: str
    expected_error: str | None = None
    expected_models: tuple[str, ...] = ()


@dataclass(frozen=True)
class MaterializationSwitchTestCase:
    """A model's materialization across builds and the relation type each build leaves."""

    description: str
    materializations: tuple[str, ...]
    expected_table_types: tuple[str, ...]
    expected_owned_view_drops: tuple[str, ...] = ()


@dataclass(frozen=True)
class RetentionLookupTestCase:
    """Many managed-retention tables in one schema and the lookups their build may issue."""

    description: str
    table_count: int
    retention_days: int
    expected_first_alters: int
    expected_max_table_lookups: int
    expected_max_listings: int
    header_options: str = ""
    expected_query_tags: int = 0
    expected_information_schema: tuple[str, ...] = ()


@dataclass(frozen=True)
class ExternalChangeTestCase:
    """A change another session makes to a model's relation while a build promotes it."""

    description: str
    builds_before: int
    change: Callable[[SimulatedSnowflakeWarehouse], None]
    before_statement: str
    expected_table_types: tuple[str, ...]


@dataclass(frozen=True)
class PlannerListingReuseTestCase:
    """Two real ``sqb build`` runs and the schema listings each issues.

    Each external change runs once, when a build reaches its statement pattern, as another
    session changing the schema between planning and the run.
    """

    description: str
    expected_listings: tuple[dict[str, int], ...]
    expected_table_types: tuple[str, ...]
    listing_max_age_seconds: float = 60.0
    external_changes: tuple[
        tuple[re.Pattern[str], Callable[[SimulatedSnowflakeWarehouse], None]], ...
    ] = ()


@dataclass(frozen=True)
class QuotedColumnBuildTestCase:
    """A source stored with quoted mixed-case column names, built twice through the real CLI."""

    description: str
    stored_columns: tuple[str, ...]
    expected_fragment: str


@dataclass(frozen=True)
class QuotedContractBuildTestCase:
    """An enforced contract declaring a name the model's quoted mixed-case alias produces."""

    description: str
    declared_name: str
    produced_columns: tuple[str, ...]
    expected_exit_code: int = 0


@dataclass(frozen=True)
class QuotedConfiguredColumnTestCase:
    """A snapshot key and a microbatch cursor configured for quoted mixed-case columns."""

    description: str
    unique_key: str
    updated_at: str
    cursor: str
    expected_exit_code: int
    expected_fragments: tuple[str, ...] = ()
    expected_errors: tuple[str, ...] = ()


@dataclass(frozen=True)
class TemporalSourceContractTestCase:
    """An enforced source contract's temporal type and the column type the warehouse holds."""

    description: str
    declared_type: str
    warehouse_type: str
    expected_models: tuple[str, ...]
    expected_error: str = ""
