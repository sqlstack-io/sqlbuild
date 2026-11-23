"""Tests for per-phase medians in the same-runner compile performance ratio guard."""

from pathlib import Path

import pytest

from scripts.compile_performance_ratio._helpers.measure import check_cache_use, median_phases
from scripts.compile_performance_ratio.constants import COLD_MODE, EDIT_MODE, WARM_MODE
from scripts.compile_performance_ratio.exceptions import CompileComparisonError
from scripts.compile_performance_ratio.models import CompileRun
from tests.unit.scripts.compile_performance_ratio._helpers._test_types import (
    CacheUseErrorTestCase,
    CacheUseTestCase,
    MedianPhasesTestCase,
    UncachedMatchTestCase,
)
from tests.unit.scripts.compile_performance_ratio._helpers.helpers import check_against_uncached


@pytest.mark.parametrize(
    "test_case",
    (
        MedianPhasesTestCase(
            description="a phase every run reports gets its median",
            runs=(
                CompileRun(
                    label="head",
                    wall_seconds=1.0,
                    cpu_seconds=1.0,
                    timings_ms={"contracts_cpu_ms": 90},
                ),
                CompileRun(
                    label="head",
                    wall_seconds=1.0,
                    cpu_seconds=1.0,
                    timings_ms={"contracts_cpu_ms": 120},
                ),
                CompileRun(
                    label="head",
                    wall_seconds=1.0,
                    cpu_seconds=1.0,
                    timings_ms={"contracts_cpu_ms": 100},
                ),
            ),
            expected_phases={"contracts_cpu_ms": 100},
        ),
        MedianPhasesTestCase(
            description="a phase one run omits is left unreported instead of counted as zero",
            runs=(
                CompileRun(
                    label="head",
                    wall_seconds=1.0,
                    cpu_seconds=1.0,
                    timings_ms={"contracts_cpu_ms": 90},
                ),
                CompileRun(label="head", wall_seconds=1.0, cpu_seconds=1.0, timings_ms={}),
            ),
            expected_phases={},
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_compile_runs_when_taking_phase_medians_then_reports_only_complete_phases(
    test_case: MedianPhasesTestCase,
) -> None:
    assert median_phases(runs=list(test_case.runs)) == test_case.expected_phases


@pytest.mark.parametrize(
    "test_case",
    (
        CacheUseTestCase(
            description="a Python-analysed edit misses the analysis cache for the edited model",
            run=CompileRun(
                label="base",
                wall_seconds=1.0,
                cpu_seconds=1.0,
                timings_ms={},
                analysis_cache_misses=1,
                analysis_cache_bypasses=0,
                analysed_models=3000,
            ),
            mode=EDIT_MODE,
        ),
        CacheUseTestCase(
            description="a delta edit analyses only the edited model, afresh",
            run=CompileRun(
                label="head",
                wall_seconds=1.0,
                cpu_seconds=1.0,
                timings_ms={},
                analysis_cache_misses=0,
                analysis_cache_bypasses=1,
                analysed_models=1,
            ),
            mode=EDIT_MODE,
        ),
        CacheUseTestCase(
            description="a warm compile that hit the analysis cache for every model",
            run=CompileRun(
                label="head",
                wall_seconds=1.0,
                cpu_seconds=1.0,
                timings_ms={},
                analysis_cache_misses=0,
                analysis_cache_bypasses=0,
            ),
            mode=WARM_MODE,
        ),
        CacheUseTestCase(
            description="a cold compile is not checked",
            run=CompileRun(
                label="head",
                wall_seconds=1.0,
                cpu_seconds=1.0,
                timings_ms={},
                analysis_cache_misses=0,
            ),
            mode=COLD_MODE,
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_observed_cache_use_when_checking_then_accepts_the_run(
    test_case: CacheUseTestCase,
) -> None:
    assert (
        check_cache_use(run=test_case.run, mode=test_case.mode, models=test_case.models)
        is test_case.expected_result
    )


@pytest.mark.parametrize(
    "test_case",
    (
        CacheUseErrorTestCase(
            description="an edit that missed no cache entry was not observed",
            run=CompileRun(
                label="head",
                wall_seconds=1.0,
                cpu_seconds=1.0,
                timings_ms={},
                analysis_cache_misses=0,
                analysis_cache_bypasses=0,
            ),
            mode=EDIT_MODE,
            expected_message="reported no analysis cache miss",
        ),
        CacheUseErrorTestCase(
            description="an edit that bypassed the cache did not measure a cached compile",
            run=CompileRun(
                label="head",
                wall_seconds=1.0,
                cpu_seconds=1.0,
                timings_ms={},
                analysis_cache_misses=0,
                analysis_cache_bypasses=3000,
                analysed_models=3000,
            ),
            mode=EDIT_MODE,
            expected_message="bypassed the analysis cache for 3000 models",
        ),
        CacheUseErrorTestCase(
            description="a delta edit that analysed nothing afresh did not observe the edit",
            run=CompileRun(
                label="head",
                wall_seconds=1.0,
                cpu_seconds=1.0,
                timings_ms={},
                analysis_cache_misses=0,
                analysis_cache_bypasses=0,
                analysed_models=1,
            ),
            mode=EDIT_MODE,
            expected_message="delta compile analysed no model afresh",
        ),
        CacheUseErrorTestCase(
            description="a warm compile that bypassed the cache, even for few models, is rejected",
            run=CompileRun(
                label="head",
                wall_seconds=1.0,
                cpu_seconds=1.0,
                timings_ms={},
                analysis_cache_misses=0,
                analysis_cache_bypasses=1,
                analysed_models=1,
            ),
            mode=WARM_MODE,
            expected_message="bypassed the analysis cache for 1 models",
        ),
        CacheUseErrorTestCase(
            description="a warm compile that missed the cache is not an unchanged compile",
            run=CompileRun(
                label="base",
                wall_seconds=1.0,
                cpu_seconds=1.0,
                timings_ms={},
                analysis_cache_misses=2,
                analysis_cache_bypasses=0,
            ),
            mode=WARM_MODE,
            expected_message="missed the analysis cache for 2 models",
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_unobserved_cache_use_when_checking_then_rejects_the_run(
    test_case: CacheUseErrorTestCase,
) -> None:
    with pytest.raises(CompileComparisonError, match=test_case.expected_message):
        check_cache_use(run=test_case.run, mode=test_case.mode, models=test_case.models)


@pytest.mark.parametrize(
    "test_case",
    (
        UncachedMatchTestCase(
            description="an edit the uncached compile reproduces",
            incremental_report='{"models": 2}',
            uncached_report='{"models": 2}',
            incremental_files={"target/compiled/models/orders.sql": "SELECT 1"},
            uncached_files={"target/compiled/models/orders.sql": "SELECT 1"},
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_an_edit_the_uncached_compile_reproduces_when_checking_then_accepts_it(
    test_case: UncachedMatchTestCase, tmp_path: Path
) -> None:
    assert (
        check_against_uncached(
            project_dir=tmp_path,
            reports=(test_case.incremental_report, test_case.uncached_report),
            files=(test_case.incremental_files, test_case.uncached_files),
        )
        is test_case.expected_result
    )


@pytest.mark.parametrize(
    "test_case",
    (
        UncachedMatchTestCase(
            description="a stale report",
            incremental_report='{"models": 2}',
            uncached_report='{"models": 3}',
            incremental_files={"target/compiled/models/orders.sql": "SELECT 1"},
            uncached_files={"target/compiled/models/orders.sql": "SELECT 1"},
            expected_message="report differs",
        ),
        UncachedMatchTestCase(
            description="a stale compiled artifact",
            incremental_report='{"models": 2}',
            uncached_report='{"models": 2}',
            incremental_files={"target/compiled/models/orders.sql": "SELECT 1"},
            uncached_files={"target/compiled/models/orders.sql": "SELECT 2"},
            expected_message="sql/models/orders.sql",
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_a_stale_incremental_compile_when_checking_against_uncached_then_rejects_it(
    test_case: UncachedMatchTestCase, tmp_path: Path
) -> None:
    with pytest.raises(CompileComparisonError, match=test_case.expected_message):
        check_against_uncached(
            project_dir=tmp_path,
            reports=(test_case.incremental_report, test_case.uncached_report),
            files=(test_case.incremental_files, test_case.uncached_files),
        )


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
