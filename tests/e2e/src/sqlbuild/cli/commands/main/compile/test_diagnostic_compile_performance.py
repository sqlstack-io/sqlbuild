"""Bound the diagnostic-only paths missed by clean-project performance guards."""

import logging
from collections import Counter
from pathlib import Path
from statistics import median
from typing import Any

import pytest

from scripts.cold_compile_performance._helpers.diagnostic_measurement import (
    measure_diagnostic_compile,
    measure_function_name_scan,
    measure_position_mapping,
)
from scripts.cold_compile_performance._helpers.diagnostic_project import (
    write_cascade_project,
    write_diagnostic_project,
    write_function_name_project,
    write_single_model_diagnostic_project,
    write_undescribed_project,
)
from tests.e2e.src.sqlbuild.cli.commands.main.compile._test_types import DiagnosticPerformanceCase

_LOGGER: logging.Logger = logging.getLogger(__name__)
pytestmark: list[pytest.MarkDecorator] = [
    pytest.mark.performance,
    pytest.mark.cold_compile_performance,
]


@pytest.mark.parametrize(
    "test_case",
    [DiagnosticPerformanceCase("large_models_many_diagnostics")],
    ids=lambda case: case.description,
)
def test_given_large_invalid_models_when_compiling_then_bounds_diagnostic_work(
    test_case: DiagnosticPerformanceCase, tmp_path: Path
) -> None:
    write_diagnostic_project(project_dir=tmp_path, depth=test_case.depth, width=test_case.width)
    for path in (tmp_path / "sql" / "models").glob("large_orders_*.sql"):
        assert len(path.read_text().splitlines()) >= 3000
    elapsed, payload = measure_diagnostic_compile(
        project_dir=tmp_path, timeout=test_case.expected_timeout_seconds
    )
    counts: Counter[tuple[str, str]] = Counter(
        (item.get("resource_name", ""), item["severity"]) for item in payload["diagnostics"]
    )
    for index in range(3):
        assert counts[(f"large_orders_{index}", "error")] >= 100
        assert f"downstream_orders_{index}" in payload["semantic_checks_partial"]
    _LOGGER.info("large diagnostic project wall=%.3fs counts=%s", elapsed, counts)
    assert elapsed < test_case.expected_max_wall_seconds


@pytest.mark.parametrize(
    "test_case",
    [DiagnosticPerformanceCase("mapping_scales_with_diagnostics", expected_max_wall_seconds=8.0)],
    ids=lambda case: case.description,
)
def test_given_many_spans_on_large_sql_when_mapping_then_scales_linearly(
    test_case: DiagnosticPerformanceCase,
) -> None:
    cold: float = measure_position_mapping(count=1)
    single: float = median(
        measure_position_mapping(count=test_case.diagnostic_count) for _ in range(3)
    )
    doubled: float = median(
        measure_position_mapping(count=test_case.diagnostic_count * 2) for _ in range(3)
    )
    _LOGGER.info("mapping cold=%.3fs N=%.3fs 2N=%.3fs", cold, single, doubled)
    assert cold + doubled < test_case.expected_max_wall_seconds
    assert doubled <= single * 2.8 + 0.1


@pytest.mark.parametrize(
    "test_case",
    [
        DiagnosticPerformanceCase(
            "many_diagnostics_in_one_model",
            diagnostic_count=1000,
            expected_max_wall_seconds=20.0,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_one_model_with_many_diagnostics_when_compiling_then_scales_linearly(
    test_case: DiagnosticPerformanceCase, tmp_path: Path
) -> None:
    timings: list[float] = []
    for multiplier in (1, 4):
        project: Path = tmp_path / f"diagnostics_{multiplier}"
        count: int = test_case.diagnostic_count * multiplier
        write_single_model_diagnostic_project(project_dir=project, diagnostics=count)
        elapsed, payload = measure_diagnostic_compile(
            project_dir=project, timeout=test_case.expected_timeout_seconds
        )
        codes: Counter[str] = Counter(item["code"] for item in payload["diagnostics"])
        assert codes == Counter({"B002": count // 2, "B218": count // 2})
        timings.append(elapsed)
    _LOGGER.info("single-model diagnostics 1x/4x wall=%s", timings)
    assert max(timings) < test_case.expected_max_wall_seconds
    assert timings[1] <= timings[0] * 4.5 + 1.0


@pytest.mark.parametrize(
    "test_case",
    [
        DiagnosticPerformanceCase(
            "many_undescribed_resources",
            diagnostic_count=500,
            expected_max_wall_seconds=15.0,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_many_undescribed_resources_when_compiling_then_scales_linearly(
    test_case: DiagnosticPerformanceCase, tmp_path: Path
) -> None:
    timings: list[float] = []
    for multiplier in (1, 4):
        project: Path = tmp_path / f"undescribed_{multiplier}"
        count: int = test_case.diagnostic_count * multiplier
        write_undescribed_project(project_dir=project, model_count=count)
        elapsed, payload = measure_diagnostic_compile(
            project_dir=project, timeout=test_case.expected_timeout_seconds
        )
        kinds: Counter[tuple[str, str]] = Counter(
            (item["code"], item["resource_type"]) for item in payload["diagnostics"]
        )
        assert kinds == Counter(
            {("P010", "model"): count, ("P010", "source"): count, ("P010", "seed"): count // 10}
        )
        timings.append(elapsed)
    _LOGGER.info("undescribed resources 1x/4x wall=%s", timings)
    assert max(timings) < test_case.expected_max_wall_seconds
    assert timings[1] <= timings[0] * 5.0 + 1.0


@pytest.mark.parametrize(
    "test_case",
    [
        DiagnosticPerformanceCase(
            "deep_poisoned_dag", depth=40, width=50, expected_max_wall_seconds=35.0
        )
    ],
    ids=lambda case: case.description,
)
def test_given_deep_poisoned_dag_when_recovering_then_bounds_scaling(
    test_case: DiagnosticPerformanceCase, tmp_path: Path
) -> None:
    timings: list[float] = []
    for multiplier in (1, 2):
        project: Path = tmp_path / f"depth_{multiplier}"
        write_cascade_project(
            project_dir=project, depth=test_case.depth * multiplier, width=test_case.width
        )
        elapsed, payload = measure_diagnostic_compile(
            project_dir=project, timeout=test_case.expected_max_wall_seconds + 5
        )
        diagnostics: list[dict[str, Any]] = payload["diagnostics"]
        assert len(diagnostics) == test_case.width
        assert all(item["code"] == "B212" for item in diagnostics)
        assert len(payload["semantic_checks_partial"]) == test_case.depth * multiplier + 1
        assert all(item.get("notes") for item in diagnostics)
        expected_note: str = f"{test_case.depth * multiplier} downstream output uses were not type-checked because of this error"
        for diagnostic in diagnostics:
            assert expected_note in diagnostic["notes"]
        timings.append(elapsed)
    _LOGGER.info("cascade depth=%d wall=%s", test_case.depth, timings)
    assert max(timings) < test_case.expected_max_wall_seconds
    assert timings[1] <= timings[0] * 3.0 + 1.0


@pytest.mark.parametrize(
    "test_case",
    [
        DiagnosticPerformanceCase(
            "unsupported_function_spellings",
            width=300,
            depth=3,
            expected_max_wall_seconds=20.0,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_many_unsupported_function_spellings_when_compiling_then_bounds_scaling(
    test_case: DiagnosticPerformanceCase, tmp_path: Path
) -> None:
    timings: list[float] = []
    for multiplier in (1, 2):
        project: Path = tmp_path / f"calls_{multiplier}"
        calls: int = test_case.width * multiplier
        write_function_name_project(project_dir=project, model_count=test_case.depth, calls=calls)
        elapsed, payload = measure_diagnostic_compile(
            project_dir=project, timeout=test_case.expected_timeout_seconds
        )
        codes: Counter[str] = Counter(item["code"] for item in payload["diagnostics"])
        assert codes["B101"] == calls * test_case.depth
        timings.append(elapsed)
    _LOGGER.info("unsupported function spellings wall=%s", timings)
    assert max(timings) < test_case.expected_max_wall_seconds
    assert timings[1] <= timings[0] * 3.0 + 1.0


@pytest.mark.parametrize(
    "test_case",
    [
        DiagnosticPerformanceCase(
            "function_name_scan_scales_linearly",
            diagnostic_count=4000,
            expected_max_wall_seconds=1.0,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_wide_sql_with_unsupported_spellings_when_scanning_then_scales_linearly(
    test_case: DiagnosticPerformanceCase,
) -> None:
    measure_function_name_scan(calls=1)
    single: float = median(
        measure_function_name_scan(calls=test_case.diagnostic_count) for _ in range(3)
    )
    doubled: float = median(
        measure_function_name_scan(calls=test_case.diagnostic_count * 2) for _ in range(3)
    )
    _LOGGER.info("function name scan N=%.3fs 2N=%.3fs", single, doubled)
    assert doubled < test_case.expected_max_wall_seconds
    assert doubled <= single * 2.8 + 0.05


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
