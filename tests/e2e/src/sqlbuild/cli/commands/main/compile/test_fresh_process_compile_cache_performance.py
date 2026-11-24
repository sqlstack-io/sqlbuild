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
    measure_runner_calibration,
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
            # Default SQL Rules scale 9.0 s by the CI cost over select = []: 1.17x cold and
            # 1.24x for the project-config edit, which reruns every rule.
            expected_cold_max_wall_seconds=10.5,
            expected_warm_max_wall_seconds=4.5,
            expected_edit_max_wall_seconds=4.5,
            expected_project_config_edit_max_wall_seconds=11.2,
            expected_max_warm_to_cold_ratio=_MAX_WARM_TO_COLD_RATIO,
            expected_max_edit_to_cold_ratio=_MAX_EDIT_TO_COLD_RATIO,
            expected_max_cache_write_cpu_overhead_ratio=_MAX_CACHE_WRITE_CPU_OVERHEAD_RATIO,
            expected_max_cache_write_wall_overhead_ratio=_MAX_CACHE_WRITE_WALL_OVERHEAD_RATIO,
            expected_max_rss_bytes=1152 * _MIB,
            expected_max_cache_bytes=96 * _MIB,
            expected_cold_fingerprint=(
                "42453f47298eb1da88c2a4224100a6231ee4527f1e437b822dcf8f1385f85b45"
            ),
            expected_leaf_edit_fingerprint=(
                "5f1c87742eb57170683b95cc95963e5ef2cdcdd68b32158322b6693e295ca818"
            ),
            expected_macro_edit_fingerprint=(
                "f0e3da7889b0d3f80f35899fd1870934fdab8d83aa49392bdc07a9533c1b0b59"
            ),
            expected_project_config_fingerprint=(
                "109c9ca710a6015f5ed4638b88cd435a231a2680b893937b1ac44aa27f3672aa"
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
            # The one select = [] guard: the same runner as its default Rules twin, previous budgets.
            expected_cold_max_wall_seconds=9.0,
            expected_warm_max_wall_seconds=4.5,
            expected_edit_max_wall_seconds=4.5,
            expected_project_config_edit_max_wall_seconds=9.0,
            expected_max_warm_to_cold_ratio=_MAX_WARM_TO_COLD_RATIO,
            expected_max_edit_to_cold_ratio=_MAX_EDIT_TO_COLD_RATIO,
            expected_max_cache_write_cpu_overhead_ratio=_MAX_CACHE_WRITE_CPU_OVERHEAD_RATIO,
            expected_max_cache_write_wall_overhead_ratio=_MAX_CACHE_WRITE_WALL_OVERHEAD_RATIO,
            expected_max_rss_bytes=1152 * _MIB,
            expected_max_cache_bytes=96 * _MIB,
            expected_cold_fingerprint=(
                "42453f47298eb1da88c2a4224100a6231ee4527f1e437b822dcf8f1385f85b45"
            ),
            expected_leaf_edit_fingerprint=(
                "5f1c87742eb57170683b95cc95963e5ef2cdcdd68b32158322b6693e295ca818"
            ),
            expected_macro_edit_fingerprint=(
                "f0e3da7889b0d3f80f35899fd1870934fdab8d83aa49392bdc07a9533c1b0b59"
            ),
            expected_project_config_fingerprint=(
                "109c9ca710a6015f5ed4638b88cd435a231a2680b893937b1ac44aa27f3672aa"
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
            # Default SQL Rules scale 14.5 s by the CI cost over select = []: 1.18x cold and
            # 1.24x for the project-config edit, which reruns every rule.
            expected_cold_max_wall_seconds=17.1,
            expected_warm_max_wall_seconds=7.0,
            expected_edit_max_wall_seconds=7.0,
            expected_project_config_edit_max_wall_seconds=18.0,
            expected_max_warm_to_cold_ratio=_MAX_WARM_TO_COLD_RATIO,
            expected_max_edit_to_cold_ratio=_MAX_EDIT_TO_COLD_RATIO,
            expected_max_cache_write_cpu_overhead_ratio=_MAX_CACHE_WRITE_CPU_OVERHEAD_RATIO,
            expected_max_cache_write_wall_overhead_ratio=_MAX_CACHE_WRITE_WALL_OVERHEAD_RATIO,
            expected_max_rss_bytes=1664 * _MIB,
            expected_max_cache_bytes=160 * _MIB,
            expected_cold_fingerprint=(
                "b495a623a346aa797c776385b70a03f09aac73589b417c08d0905e5465efc104"
            ),
            expected_leaf_edit_fingerprint=(
                "57d7e3139cfde4be15424eec7c4265db09e6ba38f5baffb3de855c5e64ec5140"
            ),
            expected_macro_edit_fingerprint=(
                "3e6923058b55a7b40af9b0d15beeb5cca25ea65c3bfa471d842a0f289684d7db"
            ),
            expected_project_config_fingerprint=(
                "a222ebd3e36feec420aaeaef5343ed663a1d2bff1b75e9a28a91890ad4b82bb9"
            ),
            macro_call_interval=6,
            scoped_macros=True,
            expected_macro_edit_misses=1,
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
            # Default SQL Rules scale 28.0 s by the CI cost over select = []: 1.20x cold and
            # 1.32x for the project-config edit, which reruns every rule.
            expected_cold_max_wall_seconds=33.6,
            expected_warm_max_wall_seconds=14.0,
            expected_edit_max_wall_seconds=13.5,
            expected_project_config_edit_max_wall_seconds=37.0,
            expected_max_warm_to_cold_ratio=_MAX_WARM_TO_COLD_RATIO,
            expected_max_edit_to_cold_ratio=_MAX_EDIT_TO_COLD_RATIO,
            expected_max_cache_write_cpu_overhead_ratio=_MAX_CACHE_WRITE_CPU_OVERHEAD_RATIO,
            expected_max_cache_write_wall_overhead_ratio=_MAX_CACHE_WRITE_WALL_OVERHEAD_RATIO,
            # Temporary memory exception (2026-10-10); remove after conversion optimisation.
            expected_max_rss_bytes=3008 * _MIB,
            expected_max_cache_bytes=320 * _MIB,
            expected_cold_fingerprint=(
                "5e0e377f023efa6b964c321e47b14d45ca20297b4a90b33d0f801287278d66ce"
            ),
            expected_leaf_edit_fingerprint=(
                "2ecc38966f7b2a03b242eab924e3d498740f338faf099db9eeedde31f8d8fe65"
            ),
            expected_macro_edit_fingerprint=(
                "3d6af38b935d24391232fcdd601ec3e17361b08d085cc681ba6cf6308e316752"
            ),
            expected_project_config_fingerprint=(
                "bcab260b5066810e4fd4c6dba9a3e9fe958b308eadc824eaded106cfff2a17dd"
            ),
            macro_call_interval=6,
            scoped_macros=True,
            expected_macro_edit_misses=1,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_semantic_project_when_compiling_across_processes_then_cache_is_incremental(
    tmp_path: Path,
    test_case: FreshProcessCompileCachePerformanceGuardTestCase,
) -> None:
    assert_required_cgroup_memory_limit()
    # Fixed work that does not depend on SQLBuild scales the wall budgets on a slower runner
    # class; the in-job ratios below and the same-runner ratio jobs catch a slower compiler.
    factor: float = measure_runner_calibration().factor
    cold_budget: float = test_case.expected_cold_max_wall_seconds * factor
    warm_budget: float = test_case.expected_warm_max_wall_seconds * factor
    edit_budget: float = test_case.expected_edit_max_wall_seconds * factor
    project_config_edit_budget: float = (
        test_case.expected_project_config_edit_max_wall_seconds * factor
    )
    print(
        f"fresh-process cache models={test_case.model_count} cold_budget={cold_budget:.2f}s "
        f"warm_budget={warm_budget:.2f}s edit_budget={edit_budget:.2f}s "
        f"project_config_edit_budget={project_config_edit_budget:.2f}s",
        flush=True,
    )
    result: FreshProcessCompileCacheBenchmarkResult = run_fresh_process_compile_cache_benchmark(
        project_dir=tmp_path / f"semantic_cache_{test_case.model_count}",
        model_count=test_case.model_count,
        source_count=test_case.source_count,
        seed_count=test_case.seed_count,
        function_count=test_case.function_count,
        macro_count=test_case.macro_count,
        test_count=test_case.test_count,
        audit_count=test_case.audit_count,
        expected_cold_max_wall_seconds=cold_budget,
        expected_warm_max_wall_seconds=warm_budget,
        expected_edit_max_wall_seconds=edit_budget,
        expected_project_config_edit_max_wall_seconds=project_config_edit_budget,
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

    assert result.cold.elapsed_seconds < cold_budget
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
        assert measurement.elapsed_seconds < warm_budget
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
        assert measurement.elapsed_seconds < edit_budget
        assert (
            measurement.elapsed_seconds / result.cold.elapsed_seconds
            <= test_case.expected_max_edit_to_cold_ratio
        )
    assert result.project_config_edit.elapsed_seconds < project_config_edit_budget
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
