"""Deferred migrate_from lookups skip selected models; an unlistable origin falls back to the current name."""

from __future__ import annotations

from pathlib import Path
from typing import Any

import pytest

from sqlbuild._native import CompilePipelineResult, PlannerInputError
from tests.integration.src.sqlbuild.compiler.planner.main._test_types import (
    DeferredMissingOriginTestCase,
    UnlistedMigrationOriginTestCase,
)
from tests.integration.src.sqlbuild.compiler.planner.main.helpers import (
    UnlistedDatabaseDuckDbAdapter,
    deferring_unlisted_adapter,
    plan_deferred_project,
)

_UNLISTED_DATABASE: str = "archive_db"
_PROJECT_FILES: dict[str, str] = {
    "sqlbuild_project.toml": (
        'name = "orders"\nadapter = "duckdb"\ndefault_target = "dev"\n\n'
        '[targets.dev]\nschema = "dev"\n\n'
        '[targets.prod]\nschema = "prod"\n'
    ),
    "models/order_ledger.sql": (
        'MODEL (description "Test model order_ledger.", materialized table, '
        f"migrate_from {_UNLISTED_DATABASE}.prod.order_history);\n\nSELECT 1 AS order_id\n"
    ),
    "models/order_report.sql": (
        'MODEL (description "Test model order_report.", materialized table);\n\n'
        'SELECT order_id FROM __ref("order_ledger")\n'
    ),
}


@pytest.mark.parametrize(
    "test_case",
    [
        DeferredMissingOriginTestCase(
            description="unlistable origin falls back to a current name the deferred target lacks",
            select=("order_report",),
            expected_missing=("order_ledger",),
            expected_unlisted_reads=1,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_unlistable_origin_of_an_unselected_upstream_when_deferring_then_s301_names_it(
    test_case: DeferredMissingOriginTestCase,
    tmp_path: Path,
    connection: Any,
) -> None:
    adapter: UnlistedDatabaseDuckDbAdapter = deferring_unlisted_adapter(
        connection=connection, unlisted_database=_UNLISTED_DATABASE
    )

    with pytest.raises(PlannerInputError) as raised:
        _ = plan_deferred_project(
            project_dir=tmp_path,
            adapter=adapter,
            project_files=_PROJECT_FILES,
            select=test_case.select,
        )

    assert raised.value.missing == test_case.expected_missing
    assert adapter.listed_databases.count(_UNLISTED_DATABASE) == test_case.expected_unlisted_reads


@pytest.mark.parametrize(
    "test_case",
    [
        UnlistedMigrationOriginTestCase(
            description="selected models are never looked up in the deferred target",
            select=("order_ledger", "order_report"),
            expected_unlisted_reads=0,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_selected_models_when_deferring_then_their_origins_are_never_looked_up(
    test_case: UnlistedMigrationOriginTestCase,
    tmp_path: Path,
    connection: Any,
) -> None:
    adapter: UnlistedDatabaseDuckDbAdapter = deferring_unlisted_adapter(
        connection=connection, unlisted_database=_UNLISTED_DATABASE
    )

    result: CompilePipelineResult = plan_deferred_project(
        project_dir=tmp_path,
        adapter=adapter,
        project_files=_PROJECT_FILES,
        select=test_case.select,
    )

    assert not tuple(
        filter(
            lambda message: "deferred target" in message,
            (warning.message for warning in result.plan_output.warnings),
        )
    )
    assert adapter.listed_databases.count(_UNLISTED_DATABASE) == test_case.expected_unlisted_reads


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
