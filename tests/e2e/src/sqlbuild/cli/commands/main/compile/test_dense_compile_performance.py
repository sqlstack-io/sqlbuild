"""Fresh-process guards for dense query graphs and the complete built-in ruleset."""

from itertools import filterfalse
from operator import attrgetter
from pathlib import Path
from typing import cast

import pytest

from scripts.cold_compile_performance._helpers.dense_project import (
    dense_model_path,
    dense_test_path,
    write_dense_compile_project,
)
from scripts.cold_compile_performance.main.require_compile_memory_scope import (
    require_compile_memory_scope,
)
from sqlbuild._native import (
    Rule,
    RulesConfig,
    build_rule_catalogue,
    load_rules_config,
    select_catalogue_rules,
)
from tests.e2e.src.sqlbuild.cli.commands.main.compile._test_types import (
    DenseCompileGuardTestCase,
    DenseDeltaEditLoopGuardTestCase,
    DenseWarmEditCompileGuardTestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.main.compile.helpers import (
    DeltaEditLoopMeasurements,
    DenseWarmEditMeasurements,
    FreshProcessCompileBenchmarkResult,
    _run_fresh_process_compile_benchmark,
    analysed_model_count,
    fresh_process_compile_cache_metrics,
    measure_model_sql_bytes,
    measure_runner_calibration,
    median_cpu_seconds,
    median_wall_seconds,
    run_dense_delta_edit_loop_benchmark,
    run_dense_warm_edit_benchmark,
)

_MIB: int = 1024 * 1024
_GIB: int = 1024 * _MIB
# A delta reruns the edited model or the edited SQL test's few models; a full compile all of them.
_MAX_DELTA_ANALYSED_MODELS: int = 10


@pytest.mark.performance
@pytest.mark.cold_compile_performance
@pytest.mark.parametrize(
    "test_case",
    (
        DenseCompileGuardTestCase(
            "dense_models_1000_all_rules",
            1000,
            6.0,
            800 * _MIB,
            "de23b14d87e954a65c2da91918fce062d158419edd63cc8cf3dc938b3e47cd44",
        ),
        DenseCompileGuardTestCase(
            "dense_models_3000_all_rules",
            3000,
            12.0,
            3 * _GIB // 2,
            "196fca3e87989eb3e89e443b7ca93bb094198538d5bde32b9d6fcca58ffc8eed",
        ),
        DenseCompileGuardTestCase(
            "dense_models_5000_all_rules",
            5000,
            18.0,
            9 * _GIB // 4,
            "3477a472fa76d6236ebd0c9758d79c18f67f1bf55e9bbec05b002ba3744cfd22",
        ),
        DenseCompileGuardTestCase(
            "dense_models_10000_all_rules",
            10000,
            35.0,
            15 * _GIB // 4,
            "f2a574af6e8068a0634fa3c078b168dff9a475ada5d71502482ad2926c90e1c3",
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_dense_project_when_compiling_cold_then_preserves_rules_semantics_and_budgets(
    test_case: DenseCompileGuardTestCase,
    tmp_path: Path,
) -> None:
    require_compile_memory_scope()
    # Fixed work that does not depend on SQLBuild scales the budget on a slower runner class.
    wall_budget: float = test_case.expected_max_wall_seconds * measure_runner_calibration().factor
    print(f"dense cold models={test_case.model_count} wall_budget={wall_budget:.2f}s", flush=True)
    project_dir: Path = tmp_path / "dense_orders"
    write_dense_compile_project(project_dir=project_dir, model_count=test_case.model_count)
    config: RulesConfig = load_rules_config(project_dir=project_dir)
    catalogue: tuple[Rule, ...] = build_rule_catalogue(
        config=config, project_dir=project_dir, include_custom=True
    )
    selected: tuple[Rule, ...] = select_catalogue_rules(
        catalogue=catalogue, config=config, project_dir=project_dir
    )
    builtin_codes: set[str] = {rule.code for rule in filterfalse(attrgetter("custom"), catalogue)}
    assert len(builtin_codes) >= 75
    assert {rule.code for rule in filterfalse(attrgetter("custom"), selected)} == builtin_codes
    assert sum(rule.custom for rule in selected) == 1
    assert not config.rule_exceptions
    assert not config.rule_ignores
    assert not config.ignore
    assert not config.thresholds
    assert measure_model_sql_bytes(project_dir) >= 7000 * test_case.model_count
    result: FreshProcessCompileBenchmarkResult = _run_fresh_process_compile_benchmark(
        project_dir=project_dir,
        label=f"dense-{test_case.model_count}",
        expected_max_wall_seconds=wall_budget,
        compile_args=("--no-cache",),
    )
    print(
        f"dense compile models={test_case.model_count} builtin_rules={len(builtin_codes)} "
        f"fingerprint={result.semantic_fingerprint}",
        flush=True,
    )
    assert result.payload["diagnostics"] == []
    assert result.payload["has_errors"] is False
    assert result.payload["summary"] == {
        "models": test_case.model_count,
        "selected_models": test_case.model_count,
        "sources": test_case.model_count * 24 // 100,
        "seeds": test_case.model_count // 20,
        "selected_seeds": test_case.model_count // 20,
        "functions": test_case.model_count // 40,
        "selected_functions": test_case.model_count // 40,
        "audits": test_case.model_count * 17 // 10,
        "tests": test_case.model_count,
        "hooks": 0,
        "execution_layers": 48,
        "errors": 0,
        "warnings": 0,
    }
    timings: dict[str, int] = cast(dict[str, int], result.payload["compile_timings"])
    assert timings["analysis_cache_bypasses"] == test_case.model_count
    assert timings["analysis_entry_cache_hits"] == timings["analysis_batch_cache_hits"] == 0
    assert timings["rule_cache_hits"] == 0
    assert timings["rule_cache_misses"] == 2 * test_case.model_count + 1
    assert timings["built_in_rules_ms"] > 0
    assert timings["custom_rules_ms"] > 0
    assert timings["custom_rules_cpu_ms"] > 0
    assert result.semantic_fingerprint == test_case.expected_fingerprint
    assert result.elapsed_seconds < wall_budget
    assert result.peak_rss_bytes < test_case.expected_max_rss_bytes


@pytest.mark.performance
@pytest.mark.cold_compile_performance
@pytest.mark.parametrize(
    "test_case",
    (
        DenseWarmEditCompileGuardTestCase(
            "dense_models_3000_warm_and_one_edit",
            3000,
            1521,
            6.0,
            6.0,
            3 * _GIB // 2,
            "196fca3e87989eb3e89e443b7ca93bb094198538d5bde32b9d6fcca58ffc8eed",
            "1afe754dd8c66bc36e38a40c66e9183ecbaf365fd8d87a784069cd615c3ce64a",
            2,
        ),
        DenseWarmEditCompileGuardTestCase(
            "dense_models_5000_warm_and_one_edit",
            5000,
            2521,
            9.0,
            9.5,
            9 * _GIB // 4,
            "3477a472fa76d6236ebd0c9758d79c18f67f1bf55e9bbec05b002ba3744cfd22",
            "dfbada4c59a3ab8ec7733784890d74c7b64e9b0adcb03364baaabef5bdba5574",
            2,
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_dense_project_when_compiling_warm_and_after_one_edit_then_matches_oracle_in_budget(
    test_case: DenseWarmEditCompileGuardTestCase,
    tmp_path: Path,
) -> None:
    require_compile_memory_scope()
    # Fixed work that does not depend on SQLBuild scales the budgets on a slower runner class;
    # the same-runner ratio jobs remain what catches a slower compiler.
    factor: float = measure_runner_calibration().factor
    warm_budget: float = test_case.expected_warm_max_seconds * factor
    edit_budget: float = test_case.expected_edit_max_seconds * factor
    print(
        f"dense models={test_case.model_count} warm_budget={warm_budget:.2f}s "
        f"edit_budget={edit_budget:.2f}s",
        flush=True,
    )
    project_dir: Path = tmp_path / "dense_orders"
    write_dense_compile_project(project_dir=project_dir, model_count=test_case.model_count)
    edited_model_path: Path = dense_model_path(
        project_dir=project_dir, index=test_case.edited_model_index
    )
    measurements: DenseWarmEditMeasurements = run_dense_warm_edit_benchmark(
        project_dir=project_dir,
        edited_model_path=edited_model_path,
        expected_warm_max_seconds=warm_budget,
        expected_edit_max_seconds=edit_budget,
    )
    labelled: tuple[tuple[str, FreshProcessCompileBenchmarkResult], ...] = (
        ("cold", measurements.cold),
        *(("warm", result) for result in measurements.warm),
        *(("edit", result) for result in measurements.edit),
        ("oracle", measurements.oracle),
    )
    for label, result in labelled:
        print(
            f"dense {label} models={test_case.model_count} wall={result.elapsed_seconds:.2f}s "
            f"cpu={result.cpu_seconds:.2f}s fingerprint={result.semantic_fingerprint}",
            flush=True,
        )
        assert result.payload["diagnostics"] == []
        assert result.payload["has_errors"] is False
        assert result.peak_rss_bytes < test_case.expected_max_rss_bytes
    assert measurements.cold.semantic_fingerprint == test_case.expected_cold_fingerprint
    for warm in measurements.warm:
        assert warm.semantic_fingerprint == test_case.expected_cold_fingerprint
        warm_timings: dict[str, int] = cast(dict[str, int], warm.payload["compile_timings"])
        batch_hits, entry_hits, misses, bypasses = fresh_process_compile_cache_metrics(warm)
        assert (batch_hits + entry_hits, misses, bypasses) == (test_case.model_count, 0, 0)
        assert warm_timings["rule_cache_misses"] == 0
    for edit in measurements.edit:
        edit_timings: dict[str, int] = cast(dict[str, int], edit.payload["compile_timings"])
        batch_hits, entry_hits, misses, bypasses = fresh_process_compile_cache_metrics(edit)
        assert 0 < misses < test_case.model_count
        assert batch_hits + entry_hits + misses == test_case.model_count
        # The edited model misses once in built-in model rules and once in SQL lint; custom
        # rules replay their recorded facts, and every other model's built-in findings stay
        # cached.
        assert edit_timings["rule_cache_misses"] == test_case.expected_edit_rule_cache_misses
    # Earlier edits write other values, so their output differs from both the cold and final edit.
    for edit in measurements.edit[:-1]:
        assert edit.semantic_fingerprint not in {
            test_case.expected_cold_fingerprint,
            test_case.expected_edit_fingerprint,
        }
    assert measurements.edit[-1].semantic_fingerprint == test_case.expected_edit_fingerprint
    assert measurements.oracle.semantic_fingerprint == test_case.expected_edit_fingerprint
    oracle_timings: dict[str, int] = cast(
        dict[str, int], measurements.oracle.payload["compile_timings"]
    )
    assert oracle_timings["rule_cache_hits"] == 0
    # Medians of same-job runs keep one slow runner moment from failing an unchanged compiler.
    assert median_wall_seconds(measurements.warm) < warm_budget
    assert median_wall_seconds(measurements.edit) < edit_budget


if __name__ == "__main__":
    raise SystemExit(pytest.main([__file__, "-n", "auto", "--dist", "loadfile"]))


@pytest.mark.performance
@pytest.mark.cold_compile_performance
@pytest.mark.parametrize(
    "test_case",
    (
        DenseDeltaEditLoopGuardTestCase(
            description="dense_models_3000_delta_edit_loop",
            model_count=3000,
            edited_model_index=1521,
            full_pairs=4,
            expected_max_full_seconds=7.0,
            expected_max_delta_seconds=2.0,
            expected_max_delta_cpu_ratio=0.30,
            expected_max_fallback_cpu_ratio=1.10,
            expected_max_rss_bytes=3 * _GIB // 2,
        ),
        DenseDeltaEditLoopGuardTestCase(
            description="dense_models_10000_delta_edit_loop",
            model_count=10000,
            edited_model_index=5021,
            full_pairs=3,
            expected_max_full_seconds=28.0,
            expected_max_delta_seconds=8.0,
            expected_max_delta_cpu_ratio=0.275,
            expected_max_fallback_cpu_ratio=1.10,
            expected_max_rss_bytes=15 * _GIB // 4,
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_dense_project_when_editing_one_file_then_delta_compiles_stay_cheap(
    test_case: DenseDeltaEditLoopGuardTestCase,
    tmp_path: Path,
) -> None:
    require_compile_memory_scope()
    project_dir: Path = tmp_path / "dense_orders"
    write_dense_compile_project(project_dir=project_dir, model_count=test_case.model_count)
    index: int = test_case.edited_model_index

    loop: DeltaEditLoopMeasurements = run_dense_delta_edit_loop_benchmark(
        project_dir=project_dir,
        edited_files=(
            dense_model_path(project_dir=project_dir, index=index),
            dense_model_path(project_dir=project_dir, index=index + 1),
            dense_test_path(project_dir=project_dir, index=index + 2),
        ),
        full_pairs=test_case.full_pairs,
        expected_max_full_seconds=test_case.expected_max_full_seconds,
        expected_max_delta_seconds=test_case.expected_max_delta_seconds,
    )

    full_cpu: float = median_cpu_seconds(loop.full)
    fallback_cpu: float = median_cpu_seconds(loop.fallbacks)
    for label, result in loop.deltas.items():
        print(
            f"delta {label} models={test_case.model_count} wall={result.elapsed_seconds:.2f}s "
            f"cpu={result.cpu_seconds:.2f}s cpu_ratio={result.cpu_seconds / full_cpu:.3f} "
            f"analysed={analysed_model_count(result)}",
            flush=True,
        )
    print(
        f"delta loop models={test_case.model_count} full_cpu={full_cpu:.2f}s "
        f"fallback_cpu={fallback_cpu:.2f}s fallback_ratio={fallback_cpu / full_cpu:.3f}",
        flush=True,
    )
    for result in (*loop.deltas.values(), *loop.fallbacks, *loop.full):
        assert result.payload["has_errors"] is False
        assert result.payload["diagnostics"] == []
        assert result.peak_rss_bytes < test_case.expected_max_rss_bytes
    for result in loop.deltas.values():
        assert 0 < analysed_model_count(result) <= _MAX_DELTA_ANALYSED_MODELS, result
        assert result.elapsed_seconds < test_case.expected_max_delta_seconds, result
        assert result.cpu_seconds <= test_case.expected_max_delta_cpu_ratio * full_cpu, result
    for result in (*loop.fallbacks, *loop.full):
        assert analysed_model_count(result) == test_case.model_count, result
    assert fallback_cpu <= test_case.expected_max_fallback_cpu_ratio * full_cpu
