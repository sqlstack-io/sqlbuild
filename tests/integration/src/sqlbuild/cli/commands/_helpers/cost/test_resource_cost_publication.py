from __future__ import annotations

from datetime import UTC, datetime, timedelta
from decimal import Decimal
from io import StringIO
from pathlib import Path
from typing import Any

import duckdb
import pytest

from sqlbuild._native import BuildCostFinalization, CostConfig, finalize_build_cost
from sqlbuild.cost.types import CostStatus
from tests.integration.src.sqlbuild.cli.commands._helpers.cost._test_types import (
    ResourceCostPublicationTestCase,
)
from tests.integration.src.sqlbuild.cli.commands._helpers.cost.helpers import (
    ScriptedCostAdapter,
    scripted_summary,
)


@pytest.mark.parametrize(
    "test_case",
    [
        ResourceCostPublicationTestCase(
            description="configured price records dollars",
            usd_per_credit_is_default=False,
            expected_run_usd=Decimal("0.36"),
        ),
        ResourceCostPublicationTestCase(
            description="default price records credits only",
            usd_per_credit_is_default=True,
            expected_run_usd=None,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_later_builds_when_publishing_cost_then_only_improved_refreshes_are_appended(
    test_case: ResourceCostPublicationTestCase,
    tmp_path: Path,
) -> None:
    warehouse: Path = tmp_path / "warehouse.duckdb"
    adapter: ScriptedCostAdapter = ScriptedCostAdapter(
        {
            "run-1": [
                scripted_summary(CostStatus.PARTIAL, 2, "0.06"),
                scripted_summary(CostStatus.COMPLETE, 4, "0.12"),
            ],
            "run-2": [scripted_summary(CostStatus.COMPLETE, 4, "0.10")],
            "run-3": [
                scripted_summary(CostStatus.PARTIAL, 3, "0.09"),
                scripted_summary(CostStatus.PARTIAL, 2, "0.06"),
            ],
            "run-4": [scripted_summary(CostStatus.COMPLETE, 4, "0.11")],
        }
    )
    started: datetime = datetime.now(UTC) - timedelta(hours=1)
    for index, run_id in enumerate(("run-1", "run-2", "run-3", "run-4")):
        _ = finalize_build_cost(
            BuildCostFinalization(
                project_dir=tmp_path,
                adapter_name="snowflake",
                adapter=adapter,
                connection_config={"database": str(warehouse)},
                target_name="dev",
                target_database="warehouse",
                run_id=run_id,
                build_status="success",
                started_at=started + timedelta(minutes=10 * index),
                completed_at=started + timedelta(minutes=10 * index + 5),
                config=CostConfig(
                    usd_per_credit=Decimal("3"),
                    usd_per_credit_is_default=test_case.usd_per_credit_is_default,
                ),
                output_stream=StringIO(),
                use_color=False,
                collect=True,
                render=False,
                had_executable_work=True,
                target_schema="main",
                orchestration_run_id=None,
            )
        )

    with duckdb.connect(str(warehouse), read_only=True) as connection:
        runs: list[tuple[Any, ...]] = connection.execute(
            "SELECT resource_name, attempt, status, statements_observed FROM main._sqlbuild_resource_costs "
            "WHERE resource_type = '_total' ORDER BY resource_name, attempt"
        ).fetchall()
        refreshed: tuple[Any, ...] | None = connection.execute(
            "SELECT CAST(estimated_usd AS DECIMAL(18, 2)), CAST(estimated_credits AS DECIMAL(18, 2)) "
            "FROM main._sqlbuild_resource_costs "
            "WHERE resource_type = '_total' AND resource_name = 'run-1' AND attempt = 2"
        ).fetchone()
        models: list[tuple[Any, ...]] = connection.execute(
            "SELECT COUNT(*) FROM main._sqlbuild_resource_costs WHERE resource_type = 'model'"
        ).fetchall()

    assert runs == [
        ("run-1", 1, "partial", 2),
        ("run-1", 2, "complete", 4),
        ("run-2", 1, "complete", 4),
        ("run-3", 1, "partial", 3),
        ("run-4", 1, "complete", 4),
    ], "the partial run-1 is refreshed once, and run-3's regressed re-read is not appended"
    assert adapter.collected == ["run-1", "run-2", "run-1", "run-3", "run-4", "run-3"]
    assert models == [(len(runs),)], "each attempt also records its model row"
    assert refreshed is not None
    assert refreshed == (test_case.expected_run_usd, Decimal("0.12"))
