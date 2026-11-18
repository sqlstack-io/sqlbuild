"""Local snapshot fingerprints do not depend on the adapter that renders fixture SQL."""

from __future__ import annotations

from pathlib import Path

import pytest

from sqlbuild._native import (
    CompiledSqlScenario,
    CompilePipelineResult,
    Dialect,
    ScenarioExecutionPlan,
    build_cli_scenario_plan,
)
from sqlbuild.adapter.contract.classes.base_adapter import BaseAdapter
from sqlbuild.adapter.discovery.main.builtins import builtin_adapter_classes
from sqlbuild.executor.scenario._helpers.snapshots.core import (
    build_scenario_snapshot_input_fingerprint,
    build_scenario_snapshot_input_specs,
)
from tests.integration.src.sqlbuild.executor.scenario._test_types import (
    ScenarioSnapshotFingerprintIntegrationTestCase,
)
from tests.integration.src.sqlbuild.executor.scenario.helpers import (
    compile_offline_snowflake_project,
    write_partial_fixture_snowflake_project,
)


@pytest.mark.parametrize(
    "test_case",
    [
        ScenarioSnapshotFingerprintIntegrationTestCase(
            description="partial fixture rendered by capture and local adapters",
            fixture_sql="SELECT 1 AS id, NULL AS label",
            helper_ctes="",
            expected_fixture_renderings=2,
            expected_fingerprints=1,
        ),
        ScenarioSnapshotFingerprintIntegrationTestCase(
            description="fixture reading a helper CTE, rendered by capture and local adapters",
            fixture_sql="SELECT h.id, NULL AS label FROM helper_ids AS h",
            helper_ctes="helper_ids AS (SELECT 1 AS id),\n",
            expected_fixture_renderings=2,
            expected_fingerprints=1,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_capture_and_local_adapters_when_fingerprinting_snapshot_then_matches(
    test_case: ScenarioSnapshotFingerprintIntegrationTestCase, tmp_path: Path
) -> None:
    write_partial_fixture_snowflake_project(
        project_dir=tmp_path, fixture_sql=test_case.fixture_sql, helper_ctes=test_case.helper_ctes
    )
    pipeline: CompilePipelineResult = compile_offline_snowflake_project(project_dir=tmp_path)
    scenario: CompiledSqlScenario = pipeline.project.sql_scenarios[0]
    capture_adapter: BaseAdapter = builtin_adapter_classes()["snowflake"]()
    adapters: tuple[BaseAdapter, ...] = (capture_adapter, builtin_adapter_classes()["duckdb"]())
    plans: tuple[ScenarioExecutionPlan, ...] = tuple(
        build_cli_scenario_plan(
            scenario=scenario,
            pipeline_result=pipeline,
            dialect=Dialect(adapter),
            project_name="shop",
            source_lexical_syntax=capture_adapter.sql_lexical_syntax,
        )
        for adapter in adapters
    )

    renderings: set[str] = {plan.fixture_plans[0].sql for plan in plans}
    fingerprints: set[str] = {
        build_scenario_snapshot_input_fingerprint(
            scenario_name=plan.name,
            input_specs=build_scenario_snapshot_input_specs(scenario_plan=plan),
            capture_adapter="snowflake",
            capture_dialect="snowflake",
        )
        for plan in plans
    }
    assert len(renderings) == test_case.expected_fixture_renderings
    assert len(fingerprints) == test_case.expected_fingerprints


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
