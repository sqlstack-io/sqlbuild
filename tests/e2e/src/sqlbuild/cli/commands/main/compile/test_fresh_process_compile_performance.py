"""Fresh-process performance guards for representative semantic projects."""

from __future__ import annotations

import logging
from pathlib import Path

import pytest

from scripts.cold_compile_performance.main.assert_required_cgroup_memory_limit import (
    assert_required_cgroup_memory_limit,
)
from tests.e2e.src.sqlbuild.cli.commands.main.compile._test_types import (
    FreshProcessCompilePerformanceGuardTestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.main.compile.helpers import (
    FreshProcessCompileBenchmarkResult,
    run_fresh_process_semantic_compile_benchmark,
)

_LOGGER: logging.Logger = logging.getLogger(__name__)
_MIB: int = 1024 * 1024
_GIB: int = 1024 * _MIB


@pytest.mark.performance
@pytest.mark.cold_compile_performance
@pytest.mark.parametrize(
    "test_case",
    [
        FreshProcessCompilePerformanceGuardTestCase(
            description="models_3000_fresh_process_semantic_compile_stays_within_budget",
            model_count=3_000,
            source_count=713,
            seed_count=141,
            function_count=71,
            macro_count=37,
            test_count=2_945,
            audit_count=5_056,
            # Default SQL Rules: 8.0 s scaled by the measured 1.14x cold cost over select = [].
            expected_max_wall_seconds=9.2,
            expected_max_rss_bytes=1088 * _MIB,
            expected_semantic_fingerprint=(
                "d935821df54043fabc3a6911c1926578178c9b744f2a589cbef99a10d5207543"
            ),
        ),
        FreshProcessCompilePerformanceGuardTestCase(
            description="models_3000_fresh_process_compiler_only_compile_stays_within_budget",
            model_count=3_000,
            source_count=713,
            seed_count=141,
            function_count=71,
            macro_count=37,
            test_count=2_945,
            audit_count=5_056,
            expected_max_wall_seconds=8.0,
            expected_max_rss_bytes=1088 * _MIB,
            expected_semantic_fingerprint=(
                "d935821df54043fabc3a6911c1926578178c9b744f2a589cbef99a10d5207543"
            ),
            compiler_only=True,
        ),
        FreshProcessCompilePerformanceGuardTestCase(
            description="models_5000_fresh_process_semantic_compile_stays_within_budget",
            model_count=5_000,
            source_count=1_189,
            seed_count=236,
            function_count=118,
            macro_count=61,
            test_count=4_908,
            audit_count=8_427,
            # Default SQL Rules: 12.5 s scaled by the measured 1.16x cold cost over select = [].
            expected_max_wall_seconds=14.5,
            expected_max_rss_bytes=1664 * _MIB,
            expected_semantic_fingerprint=(
                "874cbfd473846c6a335ec2f79cb6b7b0b77e8e8d58652815fad1e9d59d423c46"
            ),
        ),
        FreshProcessCompilePerformanceGuardTestCase(
            description="models_5000_fresh_process_compiler_only_compile_stays_within_budget",
            model_count=5_000,
            source_count=1_189,
            seed_count=236,
            function_count=118,
            macro_count=61,
            test_count=4_908,
            audit_count=8_427,
            expected_max_wall_seconds=12.5,
            expected_max_rss_bytes=1664 * _MIB,
            expected_semantic_fingerprint=(
                "874cbfd473846c6a335ec2f79cb6b7b0b77e8e8d58652815fad1e9d59d423c46"
            ),
            compiler_only=True,
        ),
        FreshProcessCompilePerformanceGuardTestCase(
            description="models_10000_fresh_process_semantic_compile_stays_within_budget",
            model_count=10_000,
            source_count=2_377,
            seed_count=471,
            function_count=236,
            macro_count=123,
            test_count=9_816,
            audit_count=16_855,
            expected_max_wall_seconds=25.5,
            expected_max_rss_bytes=int(2.5 * _GIB),
            expected_semantic_fingerprint=(
                "5ccdabeaabfdfba1ecbae80259dd3c6eb801d683df140b7fb7a249b9555308c6"
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_scaled_semantic_project_when_compiling_fresh_then_preserves_resource_budgets(
    tmp_path: Path,
    test_case: FreshProcessCompilePerformanceGuardTestCase,
) -> None:
    assert_required_cgroup_memory_limit()
    result: FreshProcessCompileBenchmarkResult = run_fresh_process_semantic_compile_benchmark(
        project_dir=tmp_path / f"semantic_{test_case.model_count}",
        model_count=test_case.model_count,
        source_count=test_case.source_count,
        seed_count=test_case.seed_count,
        function_count=test_case.function_count,
        macro_count=test_case.macro_count,
        test_count=test_case.test_count,
        audit_count=test_case.audit_count,
        expected_max_wall_seconds=test_case.expected_max_wall_seconds,
        compiler_only=test_case.compiler_only,
    )
    _LOGGER.info(
        "fresh-process semantic compile models=%d wall=%.3fs peak_rss_bytes=%d "
        "fingerprint=%s timings=%s",
        test_case.model_count,
        result.elapsed_seconds,
        result.peak_rss_bytes,
        result.semantic_fingerprint,
        result.payload["compile_timings"],
    )

    assert result.payload["has_errors"] is False
    assert result.payload["diagnostics"] == []
    assert result.payload["summary"] == {
        "models": test_case.model_count,
        "selected_models": test_case.model_count,
        "sources": test_case.source_count,
        "seeds": test_case.seed_count,
        "selected_seeds": test_case.seed_count,
        "functions": test_case.function_count,
        "selected_functions": test_case.function_count,
        "audits": test_case.audit_count,
        "tests": test_case.test_count,
        "hooks": 0,
        "execution_layers": 54,
        "errors": 0,
        "warnings": 0,
    }
    assert result.semantic_fingerprint == test_case.expected_semantic_fingerprint
    assert result.elapsed_seconds < test_case.expected_max_wall_seconds
    assert result.peak_rss_bytes < test_case.expected_max_rss_bytes
