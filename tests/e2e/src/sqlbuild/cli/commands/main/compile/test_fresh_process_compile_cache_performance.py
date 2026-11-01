"""Fresh-process cache guards for representative semantic projects."""

from __future__ import annotations

import logging
from pathlib import Path
from typing import cast

import pytest

from scripts.cold_compile_performance.main.assert_required_cgroup_memory_limit import (
    assert_required_cgroup_memory_limit,
)
from tests.e2e.src.sqlbuild.cli.commands.main.compile._test_types import (
    FreshProcessCompileCachePerformanceGuardTestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.main.compile.helpers import (
    FreshProcessCompileBenchmarkResult,
    FreshProcessCompileCacheBenchmarkResult,
    assert_complete_compile_cache_hit,
    assert_successful_compile_cache_payload,
    fresh_process_compile_cache_metrics,
    run_fresh_process_compile_cache_benchmark,
)

_LOGGER: logging.Logger = logging.getLogger(__name__)
_GIB: int = 1024 * 1024 * 1024
_MIB: int = 1024 * 1024
_MAX_WARM_TO_COLD_RATIO: float = 0.70
# The warm run after a project-wide rewrite re-reads every unchanged artifact to compare it; under
# the CI memory cap those files have already left the page cache, so it reads them back from disk.
_MAX_AFTER_PROJECT_REWRITE_TO_COLD_RATIO: float = 0.8
_MAX_EDIT_TO_COLD_RATIO: float = 0.70
# Writing the cache during a cold compile must stay a small same-runner cost over --no-cache.
# CPU time is the tight guard because it is steady on shared runners; the looser wall-time
# backstop still catches lost parallelism, lock waits and write I/O regressions.
_MAX_CACHE_WRITE_CPU_OVERHEAD_RATIO: float = 1.25
_MAX_CACHE_WRITE_WALL_OVERHEAD_RATIO: float = 1.35


@pytest.mark.performance
@pytest.mark.cache_compile_performance
@pytest.mark.parametrize(
    "test_case",
    [
        FreshProcessCompileCachePerformanceGuardTestCase(
            description="models_3000_fresh_process_compile_cache_stays_incremental",
            model_count=3_000,
            source_count=713,
            seed_count=141,
            function_count=71,
            macro_count=500,
            test_count=2_945,
            audit_count=5_056,
            # Default SQL Rules: 9.0 s scaled by the measured 1.19x cold cost over select = [].
            expected_cold_max_wall_seconds=10.75,
            expected_warm_max_wall_seconds=4.5,
            expected_edit_max_wall_seconds=4.5,
            expected_max_warm_to_cold_ratio=_MAX_WARM_TO_COLD_RATIO,
            expected_max_edit_to_cold_ratio=_MAX_EDIT_TO_COLD_RATIO,
            expected_max_cache_write_cpu_overhead_ratio=_MAX_CACHE_WRITE_CPU_OVERHEAD_RATIO,
            expected_max_cache_write_wall_overhead_ratio=_MAX_CACHE_WRITE_WALL_OVERHEAD_RATIO,
            expected_max_rss_bytes=1152 * _MIB,
            expected_max_cache_bytes=96 * _MIB,
            expected_cold_fingerprint=(
                "1914b788abcb452422eaf6c3a2c240ff390bab9530217285a5ca93334db7b6c3"
            ),
            expected_leaf_edit_fingerprint=(
                "3a9480f952f11e16177274185e8bfda7eb0e65953a8ed1c8b21ad3cdb7d0b07d"
            ),
            expected_macro_edit_fingerprint=(
                "552931889359ce882b18d4fb11fdc87341e60a8a4d2b26ad0f1635138c8b9ad8"
            ),
            expected_project_config_fingerprint=(
                "6bf4f742469eee2be7eb5bbf2411564c924011fea0bd43becd80db4027022dac"
            ),
            macro_call_interval=6,
            scoped_macros=True,
            expected_macro_edit_misses=1,
        ),
        FreshProcessCompileCachePerformanceGuardTestCase(
            description="models_3000_fresh_process_compiler_only_compile_cache_stays_incremental",
            model_count=3_000,
            source_count=713,
            seed_count=141,
            function_count=71,
            macro_count=500,
            test_count=2_945,
            audit_count=5_056,
            expected_cold_max_wall_seconds=9.0,
            expected_warm_max_wall_seconds=4.5,
            expected_edit_max_wall_seconds=4.5,
            expected_max_warm_to_cold_ratio=_MAX_WARM_TO_COLD_RATIO,
            expected_max_edit_to_cold_ratio=_MAX_EDIT_TO_COLD_RATIO,
            expected_max_cache_write_cpu_overhead_ratio=_MAX_CACHE_WRITE_CPU_OVERHEAD_RATIO,
            expected_max_cache_write_wall_overhead_ratio=_MAX_CACHE_WRITE_WALL_OVERHEAD_RATIO,
            expected_max_rss_bytes=1152 * _MIB,
            expected_max_cache_bytes=96 * _MIB,
            expected_cold_fingerprint=(
                "1914b788abcb452422eaf6c3a2c240ff390bab9530217285a5ca93334db7b6c3"
            ),
            expected_leaf_edit_fingerprint=(
                "3a9480f952f11e16177274185e8bfda7eb0e65953a8ed1c8b21ad3cdb7d0b07d"
            ),
            expected_macro_edit_fingerprint=(
                "552931889359ce882b18d4fb11fdc87341e60a8a4d2b26ad0f1635138c8b9ad8"
            ),
            expected_project_config_fingerprint=(
                "6bf4f742469eee2be7eb5bbf2411564c924011fea0bd43becd80db4027022dac"
            ),
            macro_call_interval=6,
            scoped_macros=True,
            expected_macro_edit_misses=1,
            compiler_only=True,
        ),
        FreshProcessCompileCachePerformanceGuardTestCase(
            description="models_5000_fresh_process_compile_cache_stays_incremental",
            model_count=5_000,
            source_count=1_189,
            seed_count=236,
            function_count=118,
            macro_count=834,
            test_count=4_908,
            audit_count=8_427,
            # Default SQL Rules: 14.5 s scaled by the measured 1.20x cold cost over select = [].
            expected_cold_max_wall_seconds=17.4,
            expected_warm_max_wall_seconds=7.0,
            expected_edit_max_wall_seconds=7.0,
            expected_max_warm_to_cold_ratio=_MAX_WARM_TO_COLD_RATIO,
            expected_max_edit_to_cold_ratio=_MAX_EDIT_TO_COLD_RATIO,
            expected_max_cache_write_cpu_overhead_ratio=_MAX_CACHE_WRITE_CPU_OVERHEAD_RATIO,
            expected_max_cache_write_wall_overhead_ratio=_MAX_CACHE_WRITE_WALL_OVERHEAD_RATIO,
            expected_max_rss_bytes=1664 * _MIB,
            expected_max_cache_bytes=160 * _MIB,
            expected_cold_fingerprint=(
                "2f1e5dfd29025c25e0dffc6e74f4c2be020a5ef133ecd081f44e0c55c24858b4"
            ),
            expected_leaf_edit_fingerprint=(
                "708f04a565a71b3981c9beba420aff691c8467eb30c5cdedd45c2518d45b31ba"
            ),
            expected_macro_edit_fingerprint=(
                "7dd4d45c2b21f61cbc32d33b0e0cf98721975b9f8948277cc15dc947355cfc25"
            ),
            expected_project_config_fingerprint=(
                "69f4fe914afc955d690e9c5b9074ca69dc58b8b7a0886a421b888385e6b066cb"
            ),
            macro_call_interval=6,
            scoped_macros=True,
            expected_macro_edit_misses=1,
        ),
        FreshProcessCompileCachePerformanceGuardTestCase(
            description="models_5000_fresh_process_compiler_only_compile_cache_stays_incremental",
            model_count=5_000,
            source_count=1_189,
            seed_count=236,
            function_count=118,
            macro_count=834,
            test_count=4_908,
            audit_count=8_427,
            expected_cold_max_wall_seconds=14.5,
            expected_warm_max_wall_seconds=7.0,
            expected_edit_max_wall_seconds=7.0,
            expected_max_warm_to_cold_ratio=_MAX_WARM_TO_COLD_RATIO,
            expected_max_edit_to_cold_ratio=_MAX_EDIT_TO_COLD_RATIO,
            expected_max_cache_write_cpu_overhead_ratio=_MAX_CACHE_WRITE_CPU_OVERHEAD_RATIO,
            expected_max_cache_write_wall_overhead_ratio=_MAX_CACHE_WRITE_WALL_OVERHEAD_RATIO,
            expected_max_rss_bytes=1664 * _MIB,
            expected_max_cache_bytes=160 * _MIB,
            expected_cold_fingerprint=(
                "2f1e5dfd29025c25e0dffc6e74f4c2be020a5ef133ecd081f44e0c55c24858b4"
            ),
            expected_leaf_edit_fingerprint=(
                "708f04a565a71b3981c9beba420aff691c8467eb30c5cdedd45c2518d45b31ba"
            ),
            expected_macro_edit_fingerprint=(
                "7dd4d45c2b21f61cbc32d33b0e0cf98721975b9f8948277cc15dc947355cfc25"
            ),
            expected_project_config_fingerprint=(
                "69f4fe914afc955d690e9c5b9074ca69dc58b8b7a0886a421b888385e6b066cb"
            ),
            macro_call_interval=6,
            scoped_macros=True,
            expected_macro_edit_misses=1,
            compiler_only=True,
        ),
        FreshProcessCompileCachePerformanceGuardTestCase(
            description="models_10000_fresh_process_compile_cache_stays_incremental",
            model_count=10_000,
            source_count=2_377,
            seed_count=471,
            function_count=236,
            macro_count=1_667,
            test_count=9_816,
            audit_count=16_855,
            # Default SQL Rules: 28.0 s scaled by the measured 1.25x cold cost over select = [].
            expected_cold_max_wall_seconds=35.0,
            expected_warm_max_wall_seconds=14.0,
            expected_edit_max_wall_seconds=13.5,
            expected_max_warm_to_cold_ratio=_MAX_WARM_TO_COLD_RATIO,
            expected_max_edit_to_cold_ratio=_MAX_EDIT_TO_COLD_RATIO,
            expected_max_cache_write_cpu_overhead_ratio=_MAX_CACHE_WRITE_CPU_OVERHEAD_RATIO,
            expected_max_cache_write_wall_overhead_ratio=_MAX_CACHE_WRITE_WALL_OVERHEAD_RATIO,
            # Temporary memory exception (2026-10-10); remove after conversion optimisation.
            expected_max_rss_bytes=3008 * _MIB,
            expected_max_cache_bytes=320 * _MIB,
            expected_cold_fingerprint=(
                "ecdd7f2cf961fab739aa806b6d90283916cffad2007e1e2f3ba90a466db6cb75"
            ),
            expected_leaf_edit_fingerprint=(
                "f67b73b56d7ad7677cbfa649e7933e8f294f9dd7c99f3660d7c42e1d01b804ff"
            ),
            expected_macro_edit_fingerprint=(
                "5554615dbc5fe9dbfd7555771167f3380222cb3da468b4313076adecef0ff9c3"
            ),
            expected_project_config_fingerprint=(
                "ebc15f5381746d060005ecc61f477b1b0338640198197f6788f9beab5da88db5"
            ),
            macro_call_interval=6,
            scoped_macros=True,
            expected_macro_edit_misses=1,
        ),
        FreshProcessCompileCachePerformanceGuardTestCase(
            description="models_10000_fresh_process_compiler_only_compile_cache_stays_incremental",
            model_count=10_000,
            source_count=2_377,
            seed_count=471,
            function_count=236,
            macro_count=1_667,
            test_count=9_816,
            audit_count=16_855,
            expected_cold_max_wall_seconds=28.0,
            expected_warm_max_wall_seconds=14.0,
            expected_edit_max_wall_seconds=13.5,
            expected_max_warm_to_cold_ratio=_MAX_WARM_TO_COLD_RATIO,
            expected_max_edit_to_cold_ratio=_MAX_EDIT_TO_COLD_RATIO,
            expected_max_cache_write_cpu_overhead_ratio=_MAX_CACHE_WRITE_CPU_OVERHEAD_RATIO,
            expected_max_cache_write_wall_overhead_ratio=_MAX_CACHE_WRITE_WALL_OVERHEAD_RATIO,
            # Temporary memory exception (2026-10-10); remove after conversion optimisation.
            expected_max_rss_bytes=3008 * _MIB,
            expected_max_cache_bytes=320 * _MIB,
            expected_cold_fingerprint=(
                "ecdd7f2cf961fab739aa806b6d90283916cffad2007e1e2f3ba90a466db6cb75"
            ),
            expected_leaf_edit_fingerprint=(
                "f67b73b56d7ad7677cbfa649e7933e8f294f9dd7c99f3660d7c42e1d01b804ff"
            ),
            expected_macro_edit_fingerprint=(
                "5554615dbc5fe9dbfd7555771167f3380222cb3da468b4313076adecef0ff9c3"
            ),
            expected_project_config_fingerprint=(
                "ebc15f5381746d060005ecc61f477b1b0338640198197f6788f9beab5da88db5"
            ),
            macro_call_interval=6,
            scoped_macros=True,
            expected_macro_edit_misses=1,
            compiler_only=True,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_semantic_project_when_compiling_across_processes_then_cache_is_incremental(
    tmp_path: Path,
    test_case: FreshProcessCompileCachePerformanceGuardTestCase,
) -> None:
    assert_required_cgroup_memory_limit()
    result: FreshProcessCompileCacheBenchmarkResult = run_fresh_process_compile_cache_benchmark(
        project_dir=tmp_path / f"semantic_cache_{test_case.model_count}",
        model_count=test_case.model_count,
        source_count=test_case.source_count,
        seed_count=test_case.seed_count,
        function_count=test_case.function_count,
        macro_count=test_case.macro_count,
        test_count=test_case.test_count,
        audit_count=test_case.audit_count,
        expected_cold_max_wall_seconds=test_case.expected_cold_max_wall_seconds,
        expected_warm_max_wall_seconds=test_case.expected_warm_max_wall_seconds,
        expected_edit_max_wall_seconds=test_case.expected_edit_max_wall_seconds,
        macro_call_interval=test_case.macro_call_interval,
        scoped_macros=test_case.scoped_macros,
        compiler_only=test_case.compiler_only,
    )
    measurements: dict[str, FreshProcessCompileBenchmarkResult] = {
        "cache_disabled": result.cache_disabled,
        "cold": result.cold,
        "warm": result.warm,
        "leaf_edit": result.leaf_edit,
        "after_leaf_edit": result.after_leaf_edit,
        "macro_edit": result.macro_edit,
        "after_macro_edit": result.after_macro_edit,
        "project_config_edit": result.project_config_edit,
        "after_project_config_edit": result.after_project_config_edit,
    }
    for label, measurement in measurements.items():
        _LOGGER.info(
            "fresh-process cache models=%d path=%s wall=%.3fs cpu=%.3fs peak_rss_bytes=%d "
            "fingerprint=%s metrics=%s",
            test_case.model_count,
            label,
            measurement.elapsed_seconds,
            measurement.cpu_seconds,
            measurement.peak_rss_bytes,
            measurement.semantic_fingerprint,
            fresh_process_compile_cache_metrics(measurement),
        )
    for measurement in measurements.values():
        assert_successful_compile_cache_payload(measurement=measurement, test_case=test_case)
        assert measurement.peak_rss_bytes < test_case.expected_max_rss_bytes

    assert result.cold.elapsed_seconds < test_case.expected_cold_max_wall_seconds
    cache_write_wall_ratio: float = (
        result.cold.elapsed_seconds / result.cache_disabled.elapsed_seconds
    )
    cache_write_cpu_ratio: float = result.cold.cpu_seconds / result.cache_disabled.cpu_seconds
    _LOGGER.info(
        "fresh-process cache models=%d cache-write overhead wall_ratio=%.3f cpu_ratio=%.3f",
        test_case.model_count,
        cache_write_wall_ratio,
        cache_write_cpu_ratio,
    )
    assert cache_write_cpu_ratio <= test_case.expected_max_cache_write_cpu_overhead_ratio
    assert cache_write_wall_ratio <= test_case.expected_max_cache_write_wall_overhead_ratio
    for measurement in (
        result.warm,
        result.after_leaf_edit,
        result.after_macro_edit,
        result.after_project_config_edit,
    ):
        assert measurement.elapsed_seconds < test_case.expected_warm_max_wall_seconds
    for measurement in (result.warm, result.after_leaf_edit, result.after_macro_edit):
        assert (
            measurement.elapsed_seconds / result.cold.elapsed_seconds
            <= test_case.expected_max_warm_to_cold_ratio
        )
    assert (
        result.after_project_config_edit.elapsed_seconds / result.cold.elapsed_seconds
        <= _MAX_AFTER_PROJECT_REWRITE_TO_COLD_RATIO
    )
    for measurement in (result.leaf_edit, result.macro_edit):
        assert measurement.elapsed_seconds < test_case.expected_edit_max_wall_seconds
        assert (
            measurement.elapsed_seconds / result.cold.elapsed_seconds
            <= test_case.expected_max_edit_to_cold_ratio
        )
    assert result.project_config_edit.elapsed_seconds < test_case.expected_cold_max_wall_seconds
    assert result.cache_bytes < test_case.expected_max_cache_bytes

    assert result.cache_disabled.semantic_fingerprint == test_case.expected_cold_fingerprint
    cold_timings: dict[str, int] = cast(
        dict[str, int], result.cache_disabled.payload["compile_timings"]
    )
    assert (cold_timings["rule_cache_misses"] > 0) is not test_case.compiler_only
    assert result.cold.semantic_fingerprint == test_case.expected_cold_fingerprint
    assert result.warm.semantic_fingerprint == test_case.expected_cold_fingerprint
    assert result.leaf_edit.semantic_fingerprint == test_case.expected_leaf_edit_fingerprint
    assert result.after_leaf_edit.semantic_fingerprint == test_case.expected_leaf_edit_fingerprint
    assert result.macro_edit.semantic_fingerprint == test_case.expected_macro_edit_fingerprint
    assert result.after_macro_edit.semantic_fingerprint == test_case.expected_macro_edit_fingerprint
    assert (
        result.project_config_edit.semantic_fingerprint
        == test_case.expected_project_config_fingerprint
    )
    assert (
        result.after_project_config_edit.semantic_fingerprint
        == test_case.expected_project_config_fingerprint
    )

    assert fresh_process_compile_cache_metrics(result.cache_disabled) == (
        0,
        0,
        0,
        test_case.model_count,
    )
    assert fresh_process_compile_cache_metrics(result.cold) == (0, 0, test_case.model_count, 0)
    assert fresh_process_compile_cache_metrics(result.warm) == (0, test_case.model_count, 0, 0)
    assert fresh_process_compile_cache_metrics(result.leaf_edit) == (
        0,
        test_case.model_count - 1,
        1,
        0,
    )
    assert_complete_compile_cache_hit(
        measurement=result.after_leaf_edit,
        model_count=test_case.model_count,
    )
    macro_batch_hits, macro_entry_hits, macro_misses, macro_bypasses = (
        fresh_process_compile_cache_metrics(result.macro_edit)
    )
    assert macro_batch_hits + macro_entry_hits + macro_misses == test_case.model_count
    assert (macro_batch_hits, macro_entry_hits > 0) == (0, True)
    assert macro_misses == test_case.expected_macro_edit_misses
    assert macro_bypasses == 0
    assert_complete_compile_cache_hit(
        measurement=result.after_macro_edit,
        model_count=test_case.model_count,
    )
    config_batch_hits, config_entry_hits, config_misses, config_bypasses = (
        fresh_process_compile_cache_metrics(result.project_config_edit)
    )
    assert config_batch_hits + config_entry_hits + config_misses == test_case.model_count
    assert config_misses > 0
    assert config_bypasses == 0
    assert_complete_compile_cache_hit(
        measurement=result.after_project_config_edit,
        model_count=test_case.model_count,
    )
