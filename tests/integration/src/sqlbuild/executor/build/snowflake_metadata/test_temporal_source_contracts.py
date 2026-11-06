"""Enforced Snowflake source contracts on time and timestamp columns read through metadata."""

from __future__ import annotations

from pathlib import Path

import pytest

from sqlbuild._native import CompilePipelineResult
from tests.integration.src.sqlbuild.executor.build.snowflake_metadata._test_types import (
    TemporalSourceContractTestCase,
)
from tests.integration.src.sqlbuild.executor.build.snowflake_metadata.helpers import (
    plan_offline_snowflake_project,
    shifts_warehouse,
    write_shifts_project,
)

_PLANNED: tuple[str, ...] = ("shift_report",)


@pytest.mark.parametrize(
    "test_case",
    [
        TemporalSourceContractTestCase("millisecond time", "TIME(3)", "TIME", _PLANNED),
        TemporalSourceContractTestCase("nanosecond time", "TIME(9)", "TIME", _PLANNED),
        TemporalSourceContractTestCase("time without precision", "TIME", "TIME", _PLANNED),
        TemporalSourceContractTestCase(
            "nanosecond offset timestamp", "TIMESTAMP_TZ(9)", "TIMESTAMP_TZ", _PLANNED
        ),
        TemporalSourceContractTestCase(
            "millisecond local timestamp", "TIMESTAMP_LTZ(3)", "TIMESTAMP_LTZ", _PLANNED
        ),
        TemporalSourceContractTestCase(
            "microsecond timestamp", "TIMESTAMP_NTZ(6)", "TIMESTAMP_NTZ", _PLANNED
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_enforced_temporal_source_contract_when_planning_then_precision_is_no_mismatch(
    test_case: TemporalSourceContractTestCase, tmp_path: Path
) -> None:
    write_shifts_project(project_dir=tmp_path, declared_type=test_case.declared_type)

    planned: CompilePipelineResult = plan_offline_snowflake_project(
        project_dir=tmp_path, warehouse=shifts_warehouse(test_case.warehouse_type)
    )

    assert (
        tuple(entry.name for entry in planned.plan_output.model_entries)
        == test_case.expected_models
    )


@pytest.mark.parametrize(
    "test_case",
    [
        TemporalSourceContractTestCase(
            "a time contract on a timestamp column",
            "TIME(3)",
            "TIMESTAMP_NTZ",
            (),
            r"has type TIMESTAMP_NTZ but contract declares TIME\(3\)",
        ),
        TemporalSourceContractTestCase(
            "an offset contract on a local timestamp column",
            "TIMESTAMP_TZ(9)",
            "TIMESTAMP_LTZ",
            (),
            r"has type TIMESTAMP_LTZ but contract declares TIMESTAMP_TZ\(9\)",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_enforced_temporal_source_contract_when_planning_then_another_type_mismatches(
    test_case: TemporalSourceContractTestCase, tmp_path: Path
) -> None:
    write_shifts_project(project_dir=tmp_path, declared_type=test_case.declared_type)

    with pytest.raises(Exception, match=test_case.expected_error):
        _ = plan_offline_snowflake_project(
            project_dir=tmp_path, warehouse=shifts_warehouse(test_case.warehouse_type)
        )


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
