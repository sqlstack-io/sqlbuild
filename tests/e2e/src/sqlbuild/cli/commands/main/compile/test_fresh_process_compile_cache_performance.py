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
                "286292a621494a38a59cc7afd76122f44d45f75a7f880dda5f0812bc7f44a96e"
            ),
            expected_leaf_edit_fingerprint=(
                "b05559012bba3690a2b080249a6a3003990dad864117e2a776ccb4ce7f924fea"
            ),
            expected_macro_edit_fingerprint=(
                "055f337ed937d98535ee527b8b4857bc862fff9abb599e3569fd421efb352069"
            ),
            expected_project_config_fingerprint=(
                "17dccf392d769593c6febbcbdbb53dfbcf39cd681bf57dccdbd601ae7b1ecd8e"
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
                "286292a621494a38a59cc7afd76122f44d45f75a7f880dda5f0812bc7f44a96e"
            ),
            expected_leaf_edit_fingerprint=(
                "b05559012bba3690a2b080249a6a3003990dad864117e2a776ccb4ce7f924fea"
            ),
            expected_macro_edit_fingerprint=(
                "055f337ed937d98535ee527b8b4857bc862fff9abb599e3569fd421efb352069"
            ),
            expected_project_config_fingerprint=(
                "17dccf392d769593c6febbcbdbb53dfbcf39cd681bf57dccdbd601ae7b1ecd8e"
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
                "a89cdf1a5367f9c939beb1c3d43b70381e6d9ebcc0b2d246f7db9e2051ab6ecb"
            ),
            expected_leaf_edit_fingerprint=(
                "5abf7f05d66a6275cbe3fd11ca75765fc6c565ad8827f28a999a443a59ccde48"
            ),
            expected_macro_edit_fingerprint=(
                "5a451e1b4ce399600a9fbc736b4f21af5084d6418f826f66eb03b61b37a26145"
            ),
            expected_project_config_fingerprint=(
                "a2d57bffbd6cdc1a1bc746371fc3986b17c83044d61465a7e48613dbf6b51b85"
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
                "176af67b3dea1be84c6009e0fea5558914b849abb58a1f6411e56641c9b2ec80"
            ),
            expected_leaf_edit_fingerprint=(
                "c14622c147922edb801ead1a286b1974719141dfcc08cc2d37d3f77d1270672d"
            ),
            expected_macro_edit_fingerprint=(
                "1a8849d8a3c4b2acc69041ba82680f120bf0c91086c9a2a9d8a5cdcd82a7b767"
            ),
            expected_project_config_fingerprint=(
                "2d6d158dd90ff95dc76a742bd04ffb6d6db309933d6e8826515df548d4b46cb9"
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
