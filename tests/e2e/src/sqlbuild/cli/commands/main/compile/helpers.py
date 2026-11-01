"""Helpers for compile command performance guard tests."""

from __future__ import annotations

import hashlib
import itertools
import json
import logging
import os
import random
import re
import shutil
import signal
import statistics
import subprocess
import sys
import time
import zipfile
from bisect import bisect_left
from collections.abc import Callable, Iterator
from contextlib import contextmanager, redirect_stdout
from dataclasses import replace
from functools import partial
from io import StringIO
from pathlib import Path
from types import FrameType, ModuleType
from typing import Any, NamedTuple, cast

import duckdb
import pytest

import sqlbuild._native as native_module
import sqlbuild.adapter.type_system._helpers.type_normalization as type_normalization
import sqlbuild.cli.compile_reuse._helpers.provider_settings as provider_settings
import sqlbuild.compiler.compile._helpers.macro_bridge.call_store as call_store_module
import sqlbuild.compiler.macro_bridge.classes.macro_bridge as macro_bridge_class
from scripts.cold_compile_performance.main.read_compile_measurement import read_compile_measurement
from scripts.cold_compile_performance.main.semantic_compile_fingerprint import (
    semantic_compile_fingerprint,
)
from scripts.compiler_differential.constants import FAILURE_BASE_FILES
from scripts.delta_compile_series.models import DeltaCompileOutcome
from sqlbuild._native import COMPILE_REUSE_DISABLE_ENV_VAR as REUSE_DISABLE_ENV_VAR
from sqlbuild._native import (
    DELTA_COMPILE_DISABLE_ENV_VAR,
    ContractValidationResult,
    compiler_cache_directory,
)
from sqlbuild._native import run_cli as main
from sqlbuild.adapter.contract.classes.duckdb_backed_adapter import DuckDbBackedAdapter
from sqlbuild.adapters.duckdb.classes.duckdb_adapter import DuckDbAdapter
from sqlbuild.compiler.compile.types import CompiledProjectView
from sqlbuild.compiler.macro_bridge.constants import MACRO_CALL_STORE_FILE_NAME
from sqlbuild.observability import EventDispatcher, LifecycleEvent
from tests.e2e.src.sqlbuild.cli.commands.main.compile._test_types import (
    FreshProcessCompileCachePerformanceGuardTestCase,
    IncrementalEditStep,
    SemanticCorpusCase,
    SetOperationModel,
    StaleStoreArrangement,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import prepare_inline_project, run_sqb
from tests.integration.src.sqlbuild.compiler.compile.helpers import (
    MACRO_BRIDGE_PROJECT_FILES,
    MACRO_CALL_LOG_ENV_VAR,
)
from tests.integration.src.sqlbuild.compiler.contracts.helpers import (
    contract_statuses,
)

NATIVE_ANALYSIS_STORE_FILE_NAME: str = "model-analyses.bin"
RETIRED_FACT_CACHE_DIRECTORY_NAME: str = "facts-v1"
RETIRED_REFERENCE_CACHE_DIRECTORY_NAME: str = "references-v3"
COMPILER_CACHE_DIRECTORY_NAME: str = "compiler"
RETIRED_CACHE_NAMESPACE_SUFFIXES: tuple[str, ...] = ("", "-native-v1", "-native-preview-v1")

NATIVE_REUSE_DIRECTORY_NAME: str = "project-reuse-v3"
NATIVE_REUSE_SUFFIX: str = ".store"
NATIVE_REUSE_OUTPUT_SUFFIX: str = ".output"
RETIRED_REUSE_DIRECTORY_NAME: str = "project-reuse-v1"
REUSE_LOGGER_NAME: str = "sqlbuild.compile.reuse"
_DBT_SHAPED_SQL_SIZE_PROFILE: tuple[tuple[float, int], ...] = (
    (0.50, 1_800),
    (0.75, 4_500),
    (0.90, 8_000),
    (0.95, 12_500),
    (0.99, 20_000),
    (0.992, 50_000),
    (0.994, 175_000),
    (0.999, 265_000),
    (1.0, 522_000),
)
_DBT_SHAPED_WARM_SAMPLE_COUNT: int = 3
_PERFORMANCE_SAFETY_TIMEOUT_MULTIPLIER: float = 2.0


def semantic_corpus_cases(*, group: str) -> list[dict[str, Any]]:
    path: Path = Path(__file__).parent / "fixtures" / "semantic" / "corpus.json"
    raw: dict[str, list[dict[str, Any]]] = json.loads(path.read_text(encoding="utf-8"))
    outcomes: dict[str, list[str]] = json.loads(
        path.with_name("native_outcomes.json").read_text(encoding="utf-8")
    )
    resolved: dict[str, list[dict[str, Any]]] = {name: [] for name in raw}
    overrides: dict[str, tuple[str, str]] = {}
    for outcome, identifiers in outcomes.items():
        destination, code = outcome.split(":")
        overrides.update(dict.fromkeys(identifiers, (destination, code)))
    for original_group, cases in raw.items():
        for case in cases:
            identifier: str = case["description"].split(":", maxsplit=1)[0]
            destination, code = overrides.get(identifier, (original_group, ""))
            resolved[destination].append(
                {
                    **case,
                    "expected_exit_code": int(destination in {"invalid", "pending"}),
                    "expected_codes": [code] * bool(code) or case["expected_codes"],
                    "pending_native": destination == "pending",
                }
            )
    return resolved[group]


def prepare_semantic_corpus_project(
    *, base: Path, tmp_path: Path, test_case: SemanticCorpusCase
) -> Path:
    project: Path = tmp_path / "orders_project"
    shutil.copytree(base, project)
    for relative_path, contents in test_case.repo_files.items():
        path: Path = project / relative_path
        path.write_text(contents, encoding="utf-8")
    ignored: str = ", ".join(f'"{code}"' for code in test_case.ignored_rules)
    ignore_entry: str = (
        f'\n[[rules.rule_ignores]]\nrules = [{ignored}]\npaths = ["models/**"]\n'
        'reason = "The probe SQL exercises compiler semantics, not this Rule."\n'
    )
    with (project / "sqlbuild_project.toml").open("a", encoding="utf-8") as config:
        config.write({True: ignore_entry, False: ""}[bool(test_case.ignored_rules)])
    return project


class CompileBenchmarkMeasurement(NamedTuple):
    elapsed_seconds: float
    timings_ms: dict[str, int]
    summary: dict[str, int]


def nested_coalesce_expression(*, function_depth: int) -> str:
    expression: str = "1"
    for _ in range(function_depth):
        expression = f"COALESCE({expression}, 0)"
    return expression


class LayeredProductionCompileBenchmarkResult(NamedTuple):
    cold: CompileBenchmarkMeasurement
    warm: CompileBenchmarkMeasurement
    leaf_model_edit: CompileBenchmarkMeasurement
    central_model_edit: CompileBenchmarkMeasurement
    test_edit: CompileBenchmarkMeasurement
    macro_edit: CompileBenchmarkMeasurement
    project_config_edit: CompileBenchmarkMeasurement


class SemanticCompileBenchmarkResult(NamedTuple):
    cold: CompileBenchmarkMeasurement
    warm: CompileBenchmarkMeasurement


_RETAINED_BENCHMARK_PAYLOAD_KEYS: tuple[str, ...] = (
    "compile_timings",
    "has_errors",
    "diagnostics",
    "summary",
)


class FreshProcessCompileBenchmarkResult(NamedTuple):
    elapsed_seconds: float
    peak_rss_bytes: int
    semantic_fingerprint: str
    payload: dict[str, object]
    cpu_seconds: float
    major_page_faults: int
    minor_page_faults: int

    def __repr__(self) -> str:
        return (
            f"FreshProcessCompileBenchmarkResult(elapsed_seconds={self.elapsed_seconds}, "
            f"cpu_seconds={self.cpu_seconds}, peak_rss_bytes={self.peak_rss_bytes}, "
            f"major_page_faults={self.major_page_faults}, "
            f"minor_page_faults={self.minor_page_faults}, "
            f"semantic_fingerprint={self.semantic_fingerprint!r}, "
            f"compile_timings={self.payload.get('compile_timings')!r})"
        )


class FreshProcessCompileCacheBenchmarkResult(NamedTuple):
    cache_disabled: FreshProcessCompileBenchmarkResult
    cold: FreshProcessCompileBenchmarkResult
    warm: FreshProcessCompileBenchmarkResult
    leaf_edit: FreshProcessCompileBenchmarkResult
    after_leaf_edit: FreshProcessCompileBenchmarkResult
    macro_edit: FreshProcessCompileBenchmarkResult
    after_macro_edit: FreshProcessCompileBenchmarkResult
    project_config_edit: FreshProcessCompileBenchmarkResult
    after_project_config_edit: FreshProcessCompileBenchmarkResult
    cache_bytes: int


def fresh_process_compile_cache_metrics(
    measurement: FreshProcessCompileBenchmarkResult,
) -> tuple[int, int, int, int]:
    """Return semantic analysis cache counters from one compile result."""

    timings: object = measurement.payload["compile_timings"]
    assert isinstance(timings, dict)
    timings_by_name: dict[str, object] = {str(key): value for key, value in timings.items()}

    def metric(name: str) -> int:
        value: object = timings_by_name.get(name)
        assert type(value) is int
        return value

    return (
        metric("analysis_batch_cache_hits"),
        metric("analysis_entry_cache_hits"),
        metric("analysis_cache_misses"),
        metric("analysis_cache_bypasses"),
    )


def assert_complete_compile_cache_hit(
    *, measurement: FreshProcessCompileBenchmarkResult, model_count: int
) -> None:
    """Assert every model was served by one of the analysis cache layers."""

    batch_hits, entry_hits, misses, bypasses = fresh_process_compile_cache_metrics(measurement)
    assert batch_hits + entry_hits == model_count
    assert misses == 0
    assert bypasses == 0


def assert_successful_compile_cache_payload(
    *,
    measurement: FreshProcessCompileBenchmarkResult,
    test_case: FreshProcessCompileCachePerformanceGuardTestCase,
) -> None:
    """Assert exact representative resource counts and clean diagnostics."""

    assert measurement.payload["has_errors"] is False
    assert measurement.payload["diagnostics"] == []
    assert measurement.payload["summary"] == {
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


class DbtShapedCompileBenchmarkResult(NamedTuple):
    cold_seconds: float
    warm_median_seconds: float
    warm_samples_seconds: tuple[float, ...]


def run_advanced_compile_benchmark(
    *,
    project_dir: Path,
    model_count: int,
    expected_max_seconds: float,
    scan_event_lines_per_model: int = 0,
) -> float:
    skip_actions: dict[bool, Callable[[], None]] = {
        False: _continue_compile_benchmark,
        True: _skip_compile_benchmark,
    }
    skip_actions[os.environ.get("SQLBUILD_SKIP_PERFORMANCE_TESTS") == "1"]()

    write_advanced_compile_project(
        project_dir=project_dir,
        model_count=model_count,
        scan_event_lines_per_model=scan_event_lines_per_model,
    )
    return _run_compile_benchmark(
        project_dir=project_dir,
        expected_max_seconds=expected_max_seconds,
    )


def run_dbt_shaped_compile_benchmark(
    *,
    project_dir: Path,
    model_count: int,
    expected_max_seconds: float,
    expected_warm_max_seconds: float,
) -> DbtShapedCompileBenchmarkResult:
    skip_actions: dict[bool, Callable[[], None]] = {
        False: _continue_compile_benchmark,
        True: _skip_compile_benchmark,
    }
    skip_actions[os.environ.get("SQLBUILD_SKIP_PERFORMANCE_TESTS") == "1"]()
    write_dbt_shaped_compile_project(project_dir=project_dir, model_count=model_count)
    cold_seconds: float = _run_compile_benchmark(
        project_dir=project_dir,
        expected_max_seconds=expected_max_seconds * _PERFORMANCE_SAFETY_TIMEOUT_MULTIPLIER,
    )
    warm_safety_timeout: float = expected_warm_max_seconds * _PERFORMANCE_SAFETY_TIMEOUT_MULTIPLIER
    _run_compile_benchmark(
        project_dir=project_dir,
        expected_max_seconds=warm_safety_timeout,
    )
    warm_samples: tuple[float, ...] = tuple(
        _run_compile_benchmark(
            project_dir=project_dir,
            expected_max_seconds=warm_safety_timeout,
        )
        for _ in range(_DBT_SHAPED_WARM_SAMPLE_COUNT)
    )
    return DbtShapedCompileBenchmarkResult(
        cold_seconds=cold_seconds,
        warm_median_seconds=statistics.median(warm_samples),
        warm_samples_seconds=warm_samples,
    )


def run_test_heavy_compile_benchmark(
    *,
    project_dir: Path,
    model_count: int,
    test_count: int,
    chain_depth: int,
    fixture_row_count: int,
    expected_max_seconds: float,
    expected_warm_max_seconds: float,
    expected_edit_max_seconds: float,
) -> tuple[float, float, float, float]:
    skip_actions: dict[bool, Callable[[], None]] = {
        False: _continue_compile_benchmark,
        True: _skip_compile_benchmark,
    }
    skip_actions[os.environ.get("SQLBUILD_SKIP_PERFORMANCE_TESTS") == "1"]()
    write_test_heavy_compile_project(
        project_dir=project_dir,
        model_count=model_count,
        test_count=test_count,
        chain_depth=chain_depth,
        fixture_row_count=fixture_row_count,
    )
    cold_seconds: float = _run_compile_benchmark(
        project_dir=project_dir,
        expected_max_seconds=expected_max_seconds,
    )
    warm_seconds: float = _run_compile_benchmark(
        project_dir=project_dir,
        expected_max_seconds=expected_warm_max_seconds,
    )
    _append_benchmark_edit(project_dir / "models" / "model_00000.sql", "model")
    model_edit_seconds: float = _run_compile_benchmark(
        project_dir=project_dir,
        expected_max_seconds=expected_edit_max_seconds,
    )
    _append_benchmark_edit(project_dir / "tests" / "unit" / "test_00000.sql", "test")
    test_edit_seconds: float = _run_compile_benchmark(
        project_dir=project_dir,
        expected_max_seconds=expected_edit_max_seconds,
    )
    return cold_seconds, warm_seconds, model_edit_seconds, test_edit_seconds


def run_layered_production_compile_benchmark(
    *,
    project_dir: Path,
    model_count: int,
    source_count: int,
    seed_count: int,
    function_count: int,
    macro_count: int,
    test_count: int,
    expected_cold_max_seconds: float,
    expected_warm_max_seconds: float,
    expected_edit_max_seconds: float,
    expected_config_edit_max_seconds: float,
) -> LayeredProductionCompileBenchmarkResult:
    """Measure the production-shaped cold, warm, and representative edit paths."""

    skip_actions: dict[bool, Callable[[], None]] = {
        False: _continue_compile_benchmark,
        True: _skip_compile_benchmark,
    }
    skip_actions[os.environ.get("SQLBUILD_SKIP_PERFORMANCE_TESTS") == "1"]()
    warmup_dir: Path = project_dir.parent / "compile_runtime_warmup"
    write_advanced_compile_project(project_dir=warmup_dir, model_count=32)
    _ = _run_compile_benchmark(project_dir=warmup_dir, expected_max_seconds=5.0)
    write_layered_production_compile_project(
        project_dir=project_dir,
        model_count=model_count,
        source_count=source_count,
        seed_count=seed_count,
        function_count=function_count,
        macro_count=macro_count,
        test_count=test_count,
        audit_count=_REPRESENTATIVE_AUDIT_COUNT,
    )
    cold: CompileBenchmarkMeasurement = _run_profiled_compile_benchmark(
        project_dir=project_dir,
        expected_max_seconds=expected_cold_max_seconds,
    )
    warm: CompileBenchmarkMeasurement = _run_profiled_compile_benchmark(
        project_dir=project_dir,
        expected_max_seconds=expected_warm_max_seconds,
    )
    _append_benchmark_edit(_generated_model_path(project_dir, model_count - 1), "leaf model")
    leaf_model_edit: CompileBenchmarkMeasurement = _run_profiled_compile_benchmark(
        project_dir=project_dir,
        expected_max_seconds=expected_edit_max_seconds,
    )
    _append_benchmark_edit(_generated_model_path(project_dir, 0), "central model")
    central_model_edit: CompileBenchmarkMeasurement = _run_profiled_compile_benchmark(
        project_dir=project_dir,
        expected_max_seconds=expected_edit_max_seconds,
    )
    _append_benchmark_edit(
        project_dir / "tests" / "unit" / "test_group_00000.sql",
        "test",
    )
    test_edit: CompileBenchmarkMeasurement = _run_profiled_compile_benchmark(
        project_dir=project_dir,
        expected_max_seconds=expected_edit_max_seconds,
    )
    macro_path: Path = project_dir / "macros" / "python" / "macro_00000.py"
    _replace_benchmark_text(
        path=macro_path,
        old='return f"({expression} + 0)"',
        new='return f"({expression} + 1000)"',
    )
    macro_edit: CompileBenchmarkMeasurement = _run_profiled_compile_benchmark(
        project_dir=project_dir,
        expected_max_seconds=expected_edit_max_seconds,
    )
    config_path: Path = project_dir / "sqlbuild_project.toml"
    _replace_benchmark_text(
        path=config_path,
        old='benchmark_revision = "0"',
        new='benchmark_revision = "1"',
    )
    project_config_edit: CompileBenchmarkMeasurement = _run_profiled_compile_benchmark(
        project_dir=project_dir,
        expected_max_seconds=expected_config_edit_max_seconds,
    )
    return LayeredProductionCompileBenchmarkResult(
        cold=cold,
        warm=warm,
        leaf_model_edit=leaf_model_edit,
        central_model_edit=central_model_edit,
        test_edit=test_edit,
        macro_edit=macro_edit,
        project_config_edit=project_config_edit,
    )


def run_semantic_compile_benchmark(
    *,
    project_dir: Path,
    model_count: int,
    source_count: int,
    seed_count: int,
    function_count: int,
    macro_count: int,
    test_count: int,
    audit_count: int,
    expected_cold_max_seconds: float,
    expected_warm_max_seconds: float,
) -> SemanticCompileBenchmarkResult:
    """Measure cold and unchanged compiles for a semantically dense generated project."""

    skip_actions: dict[bool, Callable[[], None]] = {
        False: _continue_compile_benchmark,
        True: _skip_compile_benchmark,
    }
    skip_actions[os.environ.get("SQLBUILD_SKIP_PERFORMANCE_TESTS") == "1"]()
    warmup_dir: Path = project_dir.parent / "semantic_compile_runtime_warmup"
    write_advanced_compile_project(project_dir=warmup_dir, model_count=32)
    _ = _run_compile_benchmark(project_dir=warmup_dir, expected_max_seconds=5.0)
    write_semantic_compile_project(
        project_dir=project_dir,
        model_count=model_count,
        source_count=source_count,
        seed_count=seed_count,
        function_count=function_count,
        macro_count=macro_count,
        test_count=test_count,
        audit_count=audit_count,
    )
    cold: CompileBenchmarkMeasurement = _run_profiled_compile_benchmark(
        project_dir=project_dir,
        expected_max_seconds=expected_cold_max_seconds,
    )
    warm: CompileBenchmarkMeasurement = _run_profiled_compile_benchmark(
        project_dir=project_dir,
        expected_max_seconds=expected_warm_max_seconds,
    )
    return SemanticCompileBenchmarkResult(cold=cold, warm=warm)


def run_fresh_process_semantic_compile_benchmark(
    *,
    project_dir: Path,
    model_count: int,
    source_count: int,
    seed_count: int,
    function_count: int,
    macro_count: int,
    test_count: int,
    audit_count: int,
    expected_max_wall_seconds: float,
    compiler_only: bool = False,
) -> FreshProcessCompileBenchmarkResult:
    """Compile one clean semantic fixture in a fresh process without compiler caches."""

    write_semantic_compile_project(
        project_dir=project_dir,
        model_count=model_count,
        source_count=source_count,
        seed_count=seed_count,
        function_count=function_count,
        macro_count=macro_count,
        test_count=test_count,
        audit_count=audit_count,
        compiler_only=compiler_only,
    )
    target_dir: Path = project_dir / "target"
    assert not target_dir.exists()
    result: FreshProcessCompileBenchmarkResult = _run_fresh_process_compile_benchmark(
        project_dir=project_dir,
        label=f"fresh-process-{model_count}",
        expected_max_wall_seconds=expected_max_wall_seconds,
        compile_args=("--no-cache",),
    )
    assert not (target_dir / "cache").exists()
    return result


def run_fresh_process_compile_cache_benchmark(
    *,
    project_dir: Path,
    model_count: int,
    source_count: int,
    seed_count: int,
    function_count: int,
    macro_count: int,
    test_count: int,
    audit_count: int,
    expected_cold_max_wall_seconds: float,
    expected_warm_max_wall_seconds: float,
    expected_edit_max_wall_seconds: float,
    macro_call_interval: int,
    scoped_macros: bool,
    compiler_only: bool = False,
) -> FreshProcessCompileCacheBenchmarkResult:
    """Measure exact and incremental cache reuse across independent CLI processes."""

    write_semantic_compile_project(
        project_dir=project_dir,
        model_count=model_count,
        source_count=source_count,
        seed_count=seed_count,
        function_count=function_count,
        macro_count=macro_count,
        test_count=test_count,
        audit_count=audit_count,
        macro_call_interval=macro_call_interval,
        scoped_macros=scoped_macros,
        compiler_only=compiler_only,
    )
    target_dir: Path = project_dir / "target"
    assert not target_dir.exists()
    cache_disabled: FreshProcessCompileBenchmarkResult = _run_fresh_process_compile_benchmark(
        project_dir=project_dir,
        label=f"cache-disabled-cold-{model_count}",
        expected_max_wall_seconds=expected_cold_max_wall_seconds,
        compile_args=("--no-cache",),
    )
    shutil.rmtree(target_dir)
    cold: FreshProcessCompileBenchmarkResult = _run_fresh_process_compile_benchmark(
        project_dir=project_dir,
        label=f"cache-cold-{model_count}",
        expected_max_wall_seconds=expected_cold_max_wall_seconds,
        compile_args=(),
    )
    warm: FreshProcessCompileBenchmarkResult = _run_fresh_process_compile_benchmark(
        project_dir=project_dir,
        label=f"cache-warm-{model_count}",
        expected_max_wall_seconds=expected_warm_max_wall_seconds,
        compile_args=(),
    )
    _append_benchmark_edit(_generated_model_path(project_dir, model_count - 1), "leaf model")
    leaf_edit: FreshProcessCompileBenchmarkResult = _run_fresh_process_compile_benchmark(
        project_dir=project_dir,
        label=f"cache-leaf-edit-{model_count}",
        expected_max_wall_seconds=expected_edit_max_wall_seconds,
        compile_args=(),
    )
    after_leaf_edit: FreshProcessCompileBenchmarkResult = _run_fresh_process_compile_benchmark(
        project_dir=project_dir,
        label=f"cache-after-leaf-edit-{model_count}",
        expected_max_wall_seconds=expected_warm_max_wall_seconds,
        compile_args=(),
    )
    macro_path: Path = next(project_dir.rglob("macro_00000.py"))
    _replace_benchmark_text(
        path=macro_path,
        old='return f"({expression} + 1)"',
        new='return f"({expression} + 1000)"',
    )
    macro_edit: FreshProcessCompileBenchmarkResult = _run_fresh_process_compile_benchmark(
        project_dir=project_dir,
        label=f"cache-macro-edit-{model_count}",
        expected_max_wall_seconds=expected_edit_max_wall_seconds,
        compile_args=(),
    )
    after_macro_edit: FreshProcessCompileBenchmarkResult = _run_fresh_process_compile_benchmark(
        project_dir=project_dir,
        label=f"cache-after-macro-edit-{model_count}",
        expected_max_wall_seconds=expected_warm_max_wall_seconds,
        compile_args=(),
    )
    project_config_path: Path = project_dir / "sqlbuild_project.toml"
    _replace_benchmark_text(
        path=project_config_path,
        old='benchmark_revision = "0"',
        new='benchmark_revision = "1"',
    )
    project_config_edit: FreshProcessCompileBenchmarkResult = _run_fresh_process_compile_benchmark(
        project_dir=project_dir,
        label=f"cache-project-config-edit-{model_count}",
        expected_max_wall_seconds=expected_cold_max_wall_seconds,
        compile_args=(),
    )
    after_project_config_edit: FreshProcessCompileBenchmarkResult = (
        _run_fresh_process_compile_benchmark(
            project_dir=project_dir,
            label=f"cache-after-project-config-edit-{model_count}",
            expected_max_wall_seconds=expected_warm_max_wall_seconds,
            compile_args=(),
        )
    )
    cache_dir: Path = compiler_cache_directory(project_dir)
    cache_bytes: int = sum(path.stat().st_size for path in cache_dir.rglob("*.json")) + sum(
        path.stat().st_size for path in cache_dir.rglob("*.sqlite3")
    )
    return FreshProcessCompileCacheBenchmarkResult(
        cache_disabled=cache_disabled,
        cold=cold,
        warm=warm,
        leaf_edit=leaf_edit,
        after_leaf_edit=after_leaf_edit,
        macro_edit=macro_edit,
        after_macro_edit=after_macro_edit,
        project_config_edit=project_config_edit,
        after_project_config_edit=after_project_config_edit,
        cache_bytes=cache_bytes,
    )


def _run_fresh_process_compile_benchmark(
    *,
    project_dir: Path,
    label: str,
    expected_max_wall_seconds: float,
    compile_args: tuple[str, ...],
    retained_payload_keys: tuple[str, ...] = _RETAINED_BENCHMARK_PAYLOAD_KEYS,
    env: dict[str, str] | None = None,
) -> FreshProcessCompileBenchmarkResult:
    run: _TimedSqbRun = _run_timed_sqb(
        project_dir=project_dir,
        label=label,
        sqb_args=("compile", "--json", *compile_args),
        expected_max_wall_seconds=expected_max_wall_seconds,
        env=env,
    )
    assert isinstance(run.payload, dict)
    payload: dict[str, object] = cast(dict[str, object], run.payload)
    compiled_dir: Path = project_dir / "target" / "compiled"
    semantic_fingerprint: str = semantic_compile_fingerprint(
        payload=payload, compiled_dir=compiled_dir
    )
    return FreshProcessCompileBenchmarkResult(
        elapsed_seconds=run.elapsed_seconds,
        peak_rss_bytes=run.peak_rss_bytes,
        semantic_fingerprint=semantic_fingerprint,
        payload={key: payload.get(key) for key in retained_payload_keys},
        cpu_seconds=run.cpu_seconds,
        major_page_faults=run.major_page_faults,
        minor_page_faults=run.minor_page_faults,
    )


class _TimedSqbRun(NamedTuple):
    elapsed_seconds: float
    peak_rss_bytes: int
    cpu_seconds: float
    major_page_faults: int
    minor_page_faults: int
    payload: object
    output_path: Path
    stderr_path: Path


def _run_timed_sqb(
    *,
    project_dir: Path,
    label: str,
    sqb_args: tuple[str, ...],
    expected_max_wall_seconds: float,
    env: dict[str, str] | None = None,
) -> _TimedSqbRun:
    measurement_path: Path = project_dir.parent / f"{label}-measurement.txt"
    output_path: Path = project_dir.parent / f"{label}.out"
    stderr_path: Path = project_dir.parent / f"{label}.stderr"
    command: list[str] = [
        "/usr/bin/time",
        "--quiet",
        "--output",
        str(measurement_path),
        "--format",
        "%e %M %U %S %F %R",
        str(Path(sys.executable).with_name("sqb")),
        "--project-dir",
        str(project_dir),
        "--no-color",
        *sqb_args,
    ]
    # Flush the previous run's artifact writes so their disk write-back is not timed here.
    os.sync()
    started: float = time.monotonic()
    try:
        with (
            output_path.open("wb") as output_file,
            stderr_path.open("wb") as stderr_file,
        ):
            with subprocess.Popen(
                command,
                stdout=output_file,
                stderr=stderr_file,
                start_new_session=True,
                env={**os.environ, **(env or {})},
            ) as process:
                try:
                    returncode: int = process.wait(timeout=expected_max_wall_seconds + 10.0)
                except subprocess.TimeoutExpired:
                    os.killpg(process.pid, signal.SIGKILL)
                    process.wait()
                    raise
    finally:
        payload, measurement = read_compile_measurement(
            label=label,
            measurement_path=measurement_path,
            output_path=output_path,
            elapsed_seconds=time.monotonic() - started,
        )
    assert returncode == 0, stderr_path.read_text(encoding="utf-8")
    elapsed_text, peak_rss_kib_text, user_text, system_text, major_text, minor_text = measurement
    return _TimedSqbRun(
        elapsed_seconds=float(elapsed_text),
        peak_rss_bytes=int(peak_rss_kib_text) * 1024,
        cpu_seconds=float(user_text) + float(system_text),
        major_page_faults=int(major_text),
        minor_page_faults=int(minor_text),
        payload=payload,
        output_path=output_path,
        stderr_path=stderr_path,
    )


class InspectionCommandMeasurement(NamedTuple):
    elapsed_seconds: float
    peak_rss_bytes: int
    cpu_seconds: float
    payload: object
    output: str
    stderr: str = ""


def run_fresh_process_inspection_command(
    *,
    project_dir: Path,
    label: str,
    sqb_args: tuple[str, ...],
    expected_max_wall_seconds: float,
) -> InspectionCommandMeasurement:
    """Run one read-only inspection command in a fresh measured process."""

    run: _TimedSqbRun = _run_timed_sqb(
        project_dir=project_dir,
        label=label,
        sqb_args=sqb_args,
        expected_max_wall_seconds=expected_max_wall_seconds,
    )
    return InspectionCommandMeasurement(
        elapsed_seconds=run.elapsed_seconds,
        peak_rss_bytes=run.peak_rss_bytes,
        cpu_seconds=run.cpu_seconds,
        payload=run.payload,
        output=run.output_path.read_text(encoding="utf-8"),
        stderr=run.stderr_path.read_text(encoding="utf-8"),
    )


_PLAN_PHASE_LINE: re.Pattern[str] = re.compile(
    r"^(Inspected warehouse state|Generated plan)\. \((\d+\.\d+)s\)$", re.MULTILINE
)


def fastest_plan_phase_seconds(*, project_dir: Path, label: str, runs: int) -> dict[str, float]:
    """Plan without the compile cache several times and keep each phase's fastest time."""

    fastest: dict[str, float] = {}
    index: int
    for index in range(runs):
        measurement: InspectionCommandMeasurement = run_fresh_process_inspection_command(
            project_dir=project_dir,
            label=f"{label}-{index}",
            sqb_args=("plan", "--json", "--no-cache"),
            expected_max_wall_seconds=120.0,
        )
        phase: str
        seconds: str
        for phase, seconds in _PLAN_PHASE_LINE.findall(measurement.stderr):
            fastest[phase] = min(float(seconds), fastest.get(phase, float(seconds)))
    return fastest


INSPECTION_DIAMOND_LAYERS: int = 16
INSPECTION_DIAMOND_WIDTH: int = 8
INSPECTION_BENCHMARK_MODEL_COUNT: int = (
    3_000 + INSPECTION_DIAMOND_LAYERS * INSPECTION_DIAMOND_WIDTH + 2
)


def write_inspection_benchmark_project(*, project_dir: Path, model_count: int = 3_000) -> None:
    """Write the semantic benchmark scaled to model_count models plus the shared lattice."""

    scale: float = model_count / 3_000
    write_semantic_compile_project(
        project_dir=project_dir,
        model_count=model_count,
        source_count=max(1, round(713 * scale)),
        seed_count=max(1, round(141 * scale)),
        function_count=max(1, round(71 * scale)),
        macro_count=max(1, round(37 * scale)),
        test_count=max(1, round(2_945 * scale)),
        audit_count=max(1, round(5_056 * scale)),
        shared_diamond_layers=INSPECTION_DIAMOND_LAYERS,
        shared_diamond_width=INSPECTION_DIAMOND_WIDTH,
    )


def prepare_inspection_benchmark_project(*, project_dir: Path) -> None:
    """Write and compile the shared-dependency benchmark, then warm lineage and scope caches."""

    write_inspection_benchmark_project(project_dir=project_dir)
    compiled: InspectionCommandMeasurement = run_fresh_process_inspection_command(
        project_dir=project_dir,
        label="inspection-compile",
        sqb_args=("compile", "--json"),
        expected_max_wall_seconds=60.0,
    )
    payload: dict[str, Any] = cast(dict[str, Any], compiled.payload)
    assert payload["diagnostics"] == []
    assert payload["summary"]["models"] == INSPECTION_BENCHMARK_MODEL_COUNT
    assert payload["summary"]["execution_layers"] == _SPINE_DEPTH + INSPECTION_DIAMOND_LAYERS + 2
    warmups: tuple[tuple[str, ...], ...] = (
        ("lineage", SHARED_DIAMOND_HUB, "--depth", "1"),
        ("scope", f"model:{SHARED_DIAMOND_ROLLUP}", "--json"),
    )
    for index, warmup_args in enumerate(warmups):
        run_fresh_process_inspection_command(
            project_dir=project_dir,
            label=f"inspection-warmup-{index}",
            sqb_args=warmup_args,
            expected_max_wall_seconds=60.0,
        )


def prepare_small_inspection_project(*, project_dir: Path, model_count: int) -> None:
    """Write and compile the semantic benchmark scaled down to model_count models."""

    scale: float = model_count / 3_000
    write_semantic_compile_project(
        project_dir=project_dir,
        model_count=model_count,
        source_count=max(1, round(713 * scale)),
        seed_count=max(1, round(141 * scale)),
        function_count=max(1, round(71 * scale)),
        macro_count=max(1, round(37 * scale)),
        test_count=max(1, round(2_945 * scale)),
        audit_count=max(1, round(5_056 * scale)),
    )
    _ = run_fresh_process_inspection_command(
        project_dir=project_dir,
        label="small-inspection-compile",
        sqb_args=("compile", "--json"),
        expected_max_wall_seconds=60.0,
    )


def run_dense_warm_edit_benchmark(
    *,
    project_dir: Path,
    edited_model_path: Path,
    expected_warm_max_seconds: float,
    expected_edit_max_seconds: float,
) -> dict[str, FreshProcessCompileBenchmarkResult]:
    """Measure cached cold, warm, and one-model edit plus a from-scratch oracle of the edit."""

    measurements: dict[str, FreshProcessCompileBenchmarkResult] = {
        "cold": _run_fresh_process_compile_benchmark(
            project_dir=project_dir,
            label="dense-cold",
            expected_max_wall_seconds=4 * expected_warm_max_seconds,
            compile_args=(),
        ),
        "warm": _run_fresh_process_compile_benchmark(
            project_dir=project_dir,
            label="dense-warm",
            expected_max_wall_seconds=expected_warm_max_seconds,
            compile_args=(),
        ),
    }
    _replace_benchmark_text(
        path=edited_model_path,
        old="COALESCE(b.amount, 0) + CAST(b.id AS DOUBLE)",
        new="COALESCE(b.amount, 1000) + CAST(b.id AS DOUBLE)",
    )
    measurements["edit"] = _run_fresh_process_compile_benchmark(
        project_dir=project_dir,
        label="dense-edit",
        expected_max_wall_seconds=expected_edit_max_seconds,
        compile_args=(),
    )
    oracle_dir: Path = project_dir.with_name(f"{project_dir.name}_oracle")
    _ = shutil.copytree(project_dir, oracle_dir, ignore=shutil.ignore_patterns("target"))
    measurements["oracle"] = _run_fresh_process_compile_benchmark(
        project_dir=oracle_dir,
        label="dense-oracle",
        expected_max_wall_seconds=4 * expected_edit_max_seconds,
        compile_args=("--no-cache",),
    )
    return measurements


class DeltaEditLoopMeasurements(NamedTuple):
    """Fresh-process compiles of an edit loop: deltas, delta fallbacks and delta-disabled ones."""

    deltas: dict[str, FreshProcessCompileBenchmarkResult]
    fallbacks: tuple[FreshProcessCompileBenchmarkResult, ...]
    full: tuple[FreshProcessCompileBenchmarkResult, ...]


DELTA_EDIT_LOOP_ENV: dict[str, str] = {REUSE_DISABLE_ENV_VAR: "0"}
_DELTA_DISABLED_ENV: dict[str, str] = {**DELTA_EDIT_LOOP_ENV, DELTA_COMPILE_DISABLE_ENV_VAR: "1"}
_WARM_UP_PAIRS: int = 1
# Each pair swaps which compile runs first, so neither always runs on a warmer system.
_ALTERNATING_CONFIG_RUNS: tuple[tuple[tuple[str, dict[str, str]], ...], ...] = (
    (("fallback", DELTA_EDIT_LOOP_ENV), ("full", _DELTA_DISABLED_ENV)),
    (("full", _DELTA_DISABLED_ENV), ("fallback", DELTA_EDIT_LOOP_ENV)),
)
_DENSE_DESCRIPTION: str = 'description "Generated dense model."'
_DENSE_EXPECTED_VALUE: re.Pattern[str] = re.compile(r"(__expected__\w+ AS \(SELECT CAST\()\d+")
_ANALYSIS_COUNTS: tuple[str, ...] = (
    "analysis_batch_cache_hits",
    "analysis_entry_cache_hits",
    "analysis_cache_misses",
    "analysis_cache_bypasses",
)


def analysed_model_count(result: FreshProcessCompileBenchmarkResult) -> int:
    """How many models the compile analysed, from cache or not; a delta analyses only its own."""

    timings: dict[str, int] = cast(dict[str, int], result.payload["compile_timings"])
    return sum(timings[name] for name in _ANALYSIS_COUNTS)


def _edit_loop_compile(
    *, project_dir: Path, label: str, max_seconds: float, env: dict[str, str]
) -> FreshProcessCompileBenchmarkResult:
    return _run_fresh_process_compile_benchmark(
        project_dir=project_dir,
        label=label,
        expected_max_wall_seconds=max_seconds,
        compile_args=(),
        env=env,
    )


def _edited_test_expectation(path: Path) -> None:
    contents: str = path.read_text(encoding="utf-8")
    edited, count = _DENSE_EXPECTED_VALUE.subn(r"\g<1>97", contents, count=1)
    assert count == 1 and edited != contents, path
    path.write_text(edited, encoding="utf-8")


def run_dense_delta_edit_loop_benchmark(
    *,
    project_dir: Path,
    edited_files: tuple[Path, Path, Path],
    full_pairs: int,
    expected_max_full_seconds: float,
    expected_max_delta_seconds: float,
) -> DeltaEditLoopMeasurements:
    """Compile, edit a body, a header and a SQL test, then edit config with and without deltas."""

    body_model, header_model, test_file = edited_files
    _ = _edit_loop_compile(
        project_dir=project_dir,
        label="delta-loop-first",
        max_seconds=2 * expected_max_full_seconds,
        env=DELTA_EDIT_LOOP_ENV,
    )
    deltas: dict[str, FreshProcessCompileBenchmarkResult] = {}
    for label, edit in (
        (
            "body",
            partial(
                _replace_benchmark_text,
                path=body_model,
                old="COALESCE(b.amount, 0) + CAST(b.id AS DOUBLE)",
                new="COALESCE(b.amount, 1000) + CAST(b.id AS DOUBLE)",
            ),
        ),
        (
            "header",
            partial(
                _replace_benchmark_text,
                path=header_model,
                old=_DENSE_DESCRIPTION,
                new='description "Edited dense model."',
            ),
        ),
        (
            "sql_test",
            partial(_edited_test_expectation, test_file),
        ),
    ):
        edit()
        deltas[label] = _edit_loop_compile(
            project_dir=project_dir,
            label=f"delta-loop-{label}",
            max_seconds=expected_max_delta_seconds,
            env=DELTA_EDIT_LOOP_ENV,
        )
    runs: dict[str, list[FreshProcessCompileBenchmarkResult]] = {"fallback": [], "full": []}
    for pair in range(_WARM_UP_PAIRS + full_pairs):
        for kind, env in _ALTERNATING_CONFIG_RUNS[pair % len(_ALTERNATING_CONFIG_RUNS)]:
            config: Path = project_dir / "sqlbuild_project.toml"
            config.write_text(
                config.read_text(encoding="utf-8") + f"# {kind} edit {pair}\n",
                encoding="utf-8",
            )
            runs[kind].append(
                _edit_loop_compile(
                    project_dir=project_dir,
                    label=f"delta-loop-{kind}-{pair}",
                    max_seconds=expected_max_full_seconds,
                    env=env,
                )
            )
    return DeltaEditLoopMeasurements(
        deltas=deltas,
        fallbacks=tuple(runs["fallback"][_WARM_UP_PAIRS:]),
        full=tuple(runs["full"][_WARM_UP_PAIRS:]),
    )


def median_cpu_seconds(results: tuple[FreshProcessCompileBenchmarkResult, ...]) -> float:
    """The median CPU seconds of same-runner compiles."""

    return statistics.median(result.cpu_seconds for result in results)


def _append_benchmark_edit(path: Path, label: str) -> None:
    path.write_text(
        path.read_text(encoding="utf-8") + f"\n-- one {label} edit\n",
        encoding="utf-8",
    )


def _replace_benchmark_text(*, path: Path, old: str, new: str) -> None:
    contents: str = path.read_text(encoding="utf-8")
    _ = contents.index(old)
    path.write_text(contents.replace(old, new, 1), encoding="utf-8")


def _run_compile_benchmark(*, project_dir: Path, expected_max_seconds: float) -> float:
    with _fail_after_seconds(expected_max_seconds):
        start: float = time.perf_counter()
        exit_code: int = main(
            [
                "--project-dir",
                str(project_dir),
                "--no-color",
                "compile",
            ]
        )
        elapsed_seconds: float = time.perf_counter() - start
    assert exit_code == 0
    return elapsed_seconds


def _run_profiled_compile_benchmark(
    *, project_dir: Path, expected_max_seconds: float
) -> CompileBenchmarkMeasurement:
    output: StringIO = StringIO()
    diagnostic_grace_seconds: float = 5.0
    with (
        _fail_after_seconds(expected_max_seconds + diagnostic_grace_seconds),
        redirect_stdout(output),
    ):
        start: float = time.perf_counter()
        exit_code: int = main(
            [
                "--project-dir",
                str(project_dir),
                "--no-color",
                "compile",
                "--json",
            ]
        )
        elapsed_seconds: float = time.perf_counter() - start
    assert exit_code == 0, output.getvalue()
    payload: dict[str, object] = json.loads(output.getvalue())
    timings: object = payload["compile_timings"]
    summary: object = payload["summary"]
    assert isinstance(timings, dict)
    assert isinstance(summary, dict)
    return CompileBenchmarkMeasurement(
        elapsed_seconds=elapsed_seconds,
        timings_ms={str(key): int(value) for key, value in timings.items()},
        summary={str(key): int(value) for key, value in summary.items()},
    )


def _generated_model_path(project_dir: Path, index: int) -> Path:
    layer_index: int = index % 10
    folder: str = {
        (True, True): "staging",
        (False, True): "intermediate",
        (False, False): "mart",
    }[(layer_index < 4, layer_index < 8)]
    return project_dir / "models" / folder / f"model_{index:05d}.sql"


def measure_model_sql_bytes(project_dir: Path) -> int:
    return sum(path.stat().st_size for path in (project_dir / "models").rglob("*.sql"))


def measure_compiled_test_sql_bytes(project_dir: Path) -> int:
    compiled_tests_dir: Path = project_dir / "target" / "compiled" / "tests"
    return sum(path.stat().st_size for path in compiled_tests_dir.rglob("*.sql"))


def measure_declared_model_columns(project_dir: Path) -> int:
    return sum(
        path.read_text(encoding="utf-8").count("(type ")
        for path in (project_dir / "models").rglob("*.sql")
    )


@contextmanager
def _fail_after_seconds(seconds: float) -> Iterator[None]:
    def _raise_timeout(signum: int, frame: FrameType | None) -> None:
        del signum, frame
        raise TimeoutError(f"compile benchmark exceeded {seconds:.1f}s budget")

    previous_handler: Any = signal.getsignal(signal.SIGALRM)
    signal.signal(signal.SIGALRM, _raise_timeout)
    signal.setitimer(signal.ITIMER_REAL, seconds)
    try:
        yield
    finally:
        signal.setitimer(signal.ITIMER_REAL, 0.0)
        signal.signal(signal.SIGALRM, previous_handler)


def write_advanced_compile_project(
    *,
    project_dir: Path,
    model_count: int,
    scan_event_lines_per_model: int = 0,
) -> None:
    models_dir: Path = project_dir / "models"
    models_dir.mkdir(parents=True)
    _write_compile_project_config(project_dir)
    base_model_sql: str = _with_reference_scan_workload(
        sql=_base_model_sql(),
        model_index=0,
        scan_event_lines=scan_event_lines_per_model,
    )
    (models_dir / "model_00000.sql").write_text(base_model_sql, encoding="utf-8")
    for index in range(1, model_count):
        model_sql: str = _with_reference_scan_workload(
            sql=_chain_model_sql(index=index),
            model_index=index,
            scan_event_lines=scan_event_lines_per_model,
        )
        (models_dir / f"model_{index:05d}.sql").write_text(model_sql, encoding="utf-8")


def write_dbt_shaped_compile_project(*, project_dir: Path, model_count: int) -> None:
    models_dir: Path = project_dir / "models"
    models_dir.mkdir(parents=True)
    _write_compile_project_config(project_dir)
    base_model_sql: str = _with_base_generated_logic(
        sql=_base_model_sql(),
        target_bytes=_sql_size_target(model_index=0, model_count=model_count),
    )
    (models_dir / "model_00000.sql").write_text(base_model_sql, encoding="utf-8")
    for index in range(1, model_count):
        model_sql: str = _with_chain_generated_logic(
            sql=_chain_model_sql(index=index),
            target_bytes=_sql_size_target(model_index=index, model_count=model_count),
        )
        (models_dir / f"model_{index:05d}.sql").write_text(model_sql, encoding="utf-8")


def write_test_heavy_compile_project(
    *,
    project_dir: Path,
    model_count: int,
    test_count: int,
    chain_depth: int,
    fixture_row_count: int,
) -> None:
    models_dir: Path = project_dir / "models"
    tests_dir: Path = project_dir / "tests" / "unit"
    models_dir.mkdir(parents=True)
    tests_dir.mkdir(parents=True)
    _write_compile_project_config(project_dir)
    for index in range(model_count):
        group_offset: int = index % chain_depth
        model_sql_builder: Callable[..., str] = {
            True: _test_heavy_base_model_sql,
            False: _test_heavy_chain_model_sql,
        }[group_offset == 0]
        model_sql: str = model_sql_builder(index=index)
        (models_dir / f"model_{index:05d}.sql").write_text(model_sql, encoding="utf-8")
    group_count: int = model_count // chain_depth
    for test_index in range(test_count):
        cases_per_target: int = 5
        group_index: int = (test_index // cases_per_target) % group_count
        base_index: int = group_index * chain_depth
        target_index: int = base_index + chain_depth - 1
        test_sql: str = _test_heavy_sql(
            base_index=base_index,
            target_index=target_index,
            fixture_row_count=fixture_row_count,
        )
        (tests_dir / f"test_{test_index:05d}.sql").write_text(test_sql, encoding="utf-8")


def _write_compile_project_config(project_dir: Path) -> None:
    (project_dir / "sqlbuild_project.toml").write_text(
        "\n".join(
            (
                'name = "performance_guard"',
                'adapter = "duckdb"',
                'default_target = "dev"',
                "",
                "[connection]",
                'database = ":memory:"',
                "",
                "[targets.dev]",
                'schema = "main"',
                "",
                "[[rules.rule_ignores]]",
                'rules = ["SQLCLEAN018"]',
                'paths = ["models/**"]',
                'reason = "The generated workload ranks by a non-unique id; keep its SQL stable."',
                "",
            )
        ),
        encoding="utf-8",
    )


def _continue_compile_benchmark() -> None:
    return None


def _skip_compile_benchmark() -> None:
    pytest.skip("SQLBUILD_SKIP_PERFORMANCE_TESTS=1")


def _with_reference_scan_workload(*, sql: str, model_index: int, scan_event_lines: int) -> str:
    comments: str = "".join(
        f"-- generated mapping {line:05d}: source='source_{model_index:05d}' "
        f"expression=__not_a_reference_{line:05d}\n"
        for line in range(scan_event_lines)
    )
    header_end: int = sql.index(";\n") + 2
    return f"{sql[:header_end]}{comments}{sql[header_end:]}"


def _sql_size_target(*, model_index: int, model_count: int) -> int:
    quantile: float = (model_index + 1) / model_count
    upper_quantiles: tuple[float, ...] = tuple(item[0] for item in _DBT_SHAPED_SQL_SIZE_PROFILE)
    target_sizes: tuple[int, ...] = tuple(item[1] for item in _DBT_SHAPED_SQL_SIZE_PROFILE)
    return target_sizes[bisect_left(upper_quantiles, quantile)]


def _generated_case_logic(*, sql: str, target_bytes: int) -> str:
    missing_bytes: int = max(0, target_bytes - len(sql.encode("utf-8")))
    clause_template: str = "      WHEN id = 00000 THEN 'segment_00000'\n"
    clause_count: int = (missing_bytes + len(clause_template) - 1) // len(clause_template)
    clauses: str = "".join(
        f"      WHEN id = {index:05d} THEN 'segment_{index:05d}'\n" for index in range(clause_count)
    )
    return f"    CASE\n{clauses}      ELSE 'unmatched'\n    END AS generated_mapping"


def _with_base_generated_logic(*, sql: str, target_bytes: int) -> str:
    header, query = sql.split(";\n", 1)
    generated_logic: str = _generated_case_logic(sql=sql, target_bytes=target_bytes)
    return f"""{header};
WITH base AS (
{query.rstrip()}
),
generated_logic AS (
  SELECT
    *,
{generated_logic}
  FROM base
)
SELECT id, bucket, amount, avg_amount, max_amount, status
FROM generated_logic
"""


def _with_chain_generated_logic(*, sql: str, target_bytes: int) -> str:
    generated_logic: str = _generated_case_logic(sql=sql, target_bytes=target_bytes)
    widened_sql: str = sql.replace(
        ")\nSELECT\n  id,",
        f"""),
generated_logic AS (
  SELECT
    *,
{generated_logic}
  FROM joined
)
SELECT
  id,""",
        1,
    )
    prefix, suffix = widened_sql.rsplit("\nFROM joined", 1)
    return f"{prefix}\nFROM generated_logic{suffix}"


def _base_model_sql() -> str:
    return "\n".join(
        (
            "MODEL (description 'Test model.', materialized view);",
            "",
            "SELECT",
            "  0 AS id,",
            "  'even' AS bucket,",
            "  CAST(1 AS DOUBLE) AS amount,",
            "  CAST(1 AS DOUBLE) AS avg_amount,",
            "  CAST(1 AS DOUBLE) AS max_amount,",
            "  'small' AS status",
            "",
        )
    )


def _chain_model_sql(*, index: int) -> str:
    previous_model: str = f"model_{index - 1:05d}"
    return f'''MODEL (description "Test model.", materialized view);

WITH base AS (
  SELECT
    id + 1 AS id,
    bucket,
    CAST(amount AS DOUBLE) AS amount,
    CAST(avg_amount AS DOUBLE) AS avg_amount,
    CAST(max_amount AS DOUBLE) AS max_amount,
    status
  FROM __ref("{previous_model}")
),
windowed AS (
  SELECT
    id,
    CASE WHEN id % 2 = 0 THEN 'even' ELSE 'odd' END AS bucket,
    amount + avg_amount AS amount,
    LAG(amount, 1, 0) OVER (ORDER BY id) AS previous_amount,
    SUM(amount) OVER (
      PARTITION BY bucket
      ORDER BY id
      ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW
    ) AS running_amount,
    ROW_NUMBER() OVER (PARTITION BY bucket ORDER BY id) AS row_number
  FROM base
),
grouped AS (
  SELECT
    bucket,
    AVG(amount) AS avg_amount,
    MAX(running_amount) AS max_amount,
    COUNT(*) AS row_count
  FROM windowed
  GROUP BY bucket
),
joined AS (
  SELECT
    w.id,
    w.bucket,
    w.amount + COALESCE(w.previous_amount, 0) AS amount,
    g.avg_amount,
    g.max_amount,
    CASE
      WHEN g.row_count > 10 THEN 'large'
      WHEN w.amount > g.avg_amount THEN 'above_average'
      ELSE 'small'
    END AS status
  FROM windowed w
  JOIN grouped g
    ON w.bucket = g.bucket
  WHERE w.row_number >= 1
)
SELECT
  id,
  bucket,
  amount,
  avg_amount,
  max_amount,
  status
FROM joined
'''


def _test_heavy_base_model_sql(*, index: int) -> str:
    return f"""MODEL (description "Test model.", materialized view);

SELECT
  {index} AS id,
  CAST({index} AS DOUBLE) AS amount,
  'base' AS status
"""


def _test_heavy_chain_model_sql(*, index: int) -> str:
    previous_model: str = f"model_{index - 1:05d}"
    return f'''MODEL (description "Test model.", materialized view);

WITH transformed AS (
  SELECT
    id + 1 AS id,
    amount + {index} AS amount,
    CASE WHEN id % 2 = 0 THEN 'even' ELSE 'odd' END AS status
  FROM __ref("{previous_model}")
),
windowed AS (
  SELECT
    id,
    amount,
    status,
    ROW_NUMBER() OVER (PARTITION BY status ORDER BY id) AS row_number,
    SUM(amount) OVER (PARTITION BY status ORDER BY id) AS running_amount
  FROM transformed
)
SELECT id, amount + running_amount AS amount, status
FROM windowed
WHERE row_number >= 1
'''


def _test_heavy_sql(*, base_index: int, target_index: int, fixture_row_count: int) -> str:
    fixture_rows: str = " UNION ALL\n".join(
        f"  SELECT {row} AS id, CAST({row} AS DOUBLE) AS amount, 'base' AS status"
        for row in range(fixture_row_count)
    )
    return f"""TEST();

WITH
__ref__model_{base_index:05d} AS (
{fixture_rows}
),
__expected__model_{target_index:05d} AS (
  SELECT 1 AS id, CAST(1 AS DOUBLE) AS amount, 'odd' AS status
)
SELECT 1
"""


_SPINE_DEPTH: int = 54
_TEST_CHAIN_DEPTH: int = 8
_REPRESENTATIVE_AUDIT_COUNT: int = 700
_TOP_LEVEL_WITH_INTERVAL: int = 20
_NESTED_QUERY_INTERVAL: int = 15
_MACRO_INTERVAL: int = 13
_BENCHMARK_MACRO_CALL: re.Pattern[str] = re.compile(r"@py\.(macro_\d{5})\(")
_FUNCTION_INTERVAL: int = 43
_SEED_INTERVAL: int = 67
_SEED_REFERENCE_START_INDEX: int = 300
_SQL_SIZE_PROFILE: tuple[tuple[float, int], ...] = (
    (0.0, 400),
    (0.50, 1_900),
    (0.75, 4_500),
    (0.90, 11_000),
    (0.95, 15_200),
    (0.99, 48_200),
    (0.995, 120_000),
    (0.999, 260_000),
    (1.0, 520_000),
)
_SEMANTIC_COLUMN_COUNT_PROFILE: tuple[tuple[float, int], ...] = (
    (0.0, 3),
    (0.25, 9),
    (0.50, 19),
    (0.75, 33),
    (0.90, 58),
    (0.95, 115),
    (0.99, 232),
    (1.0, 538),
)
_SEMANTIC_SET_OPERATION_MODEL_INDEX: int = _SPINE_DEPTH
_SEMANTIC_SET_OPERATION_BRANCH_COUNT: int = 267


def write_layered_production_compile_project(
    *,
    project_dir: Path,
    model_count: int,
    source_count: int,
    seed_count: int,
    function_count: int,
    macro_count: int,
    test_count: int,
    audit_count: int,
) -> None:
    """Write a deterministic generated project matching real resource ratios."""

    _layered_write_project_config(project_dir=project_dir)
    _layered_write_sources(project_dir=project_dir, source_count=source_count)
    _layered_write_seeds(project_dir=project_dir, seed_count=seed_count)
    _layered_write_functions(project_dir=project_dir, function_count=function_count)
    _layered_write_macros(project_dir=project_dir, macro_count=macro_count)
    _layered_write_schemas(project_dir=project_dir)
    _layered_write_models(
        project_dir=project_dir,
        model_count=model_count,
        source_count=source_count,
        seed_count=seed_count,
        function_count=function_count,
        macro_count=macro_count,
        audit_count=audit_count,
    )
    _layered_write_tests(
        project_dir=project_dir,
        model_count=model_count,
        test_count=test_count,
        source_count=source_count,
        seed_count=seed_count,
    )


def write_semantic_compile_project(
    *,
    project_dir: Path,
    model_count: int,
    source_count: int,
    seed_count: int,
    function_count: int,
    macro_count: int,
    test_count: int,
    audit_count: int,
    macro_call_interval: int = _MACRO_INTERVAL,
    scoped_macros: bool = False,
    shared_diamond_layers: int = 0,
    shared_diamond_width: int = 0,
    compiler_only: bool = False,
) -> None:
    """Write a neutral project with broad resources and dense SQL semantics.

    A positive `shared_diamond_layers` adds the shared-dependency lattice from
    `write_shared_diamond_models`; the default leaves the compile guard fixtures unchanged.
    `compiler_only` sets `select = []` so a guard measures the compiler without default Rules.
    """

    _layered_write_project_config(project_dir=project_dir, compiler_only=compiler_only)
    _layered_write_sources(project_dir=project_dir, source_count=source_count)
    _layered_write_seeds(project_dir=project_dir, seed_count=seed_count)
    _layered_write_functions(project_dir=project_dir, function_count=function_count)
    _layered_write_macros(project_dir=project_dir, macro_count=macro_count)
    _semantic_write_models(
        project_dir=project_dir,
        model_count=model_count,
        source_count=source_count,
        seed_count=seed_count,
        function_count=function_count,
        macro_count=macro_count,
        audit_count=audit_count,
        macro_call_interval=macro_call_interval,
    )
    diamond_writers: dict[bool, Callable[[], None]] = {
        True: lambda: write_shared_diamond_models(
            project_dir=project_dir,
            layer_count=shared_diamond_layers,
            width=shared_diamond_width,
        ),
        False: lambda: None,
    }
    diamond_writers[shared_diamond_layers > 0]()
    {True: _scope_single_folder_macros, False: _keep_project_macros}[scoped_macros](
        project_dir=project_dir
    )
    _layered_write_tests(
        project_dir=project_dir,
        model_count=model_count,
        test_count=test_count,
        source_count=source_count,
        semantic_fixture_scale=True,
        seed_count=seed_count,
    )


def _layered_write_project_config(*, project_dir: Path, compiler_only: bool = False) -> None:
    project_dir.mkdir(parents=True)
    rules_selection: str = {True: "[rules]\nselect = []\n\n", False: ""}[compiler_only]
    (project_dir / "sqlbuild_project.toml").write_text(
        f"""name = "layered_production_performance_guard"
adapter = "duckdb"
default_target = "dev"

[connections.local]
database = ":memory:"

[settings]
column_contract_mode = "explicit"

[vars]
benchmark_revision = "0"

[targets.dev]
schema = "main"

[path_defaults."staging"]
materialized = "view"

[path_defaults."intermediate"]
materialized = "table"

[path_defaults."mart"]
materialized = "table"

{rules_selection}[[rules.rule_ignores]]
rules = ["SQLCLEAN039"]
paths = ["models/**"]
reason = "The workload mixes nested query shapes on purpose to cover their compile path."
""",
        encoding="utf-8",
    )


def _layered_write_sources(*, project_dir: Path, source_count: int) -> None:
    sources_dir: Path = project_dir / "sources"
    sources_dir.mkdir()
    entries: str = "\n".join(
        f"""  - name: source_{index:05d}
    description: Test source source_{index:05d}.
    expression: "(SELECT {index} AS id, CAST({index} AS DOUBLE) AS amount, 'source' AS status)"
    columns:
      - name: id
        type: INTEGER
      - name: amount
        type: DOUBLE
      - name: status
        type: VARCHAR"""
        for index in range(source_count)
    )
    (sources_dir / "generated.yml").write_text(f"sources:\n{entries}\n", encoding="utf-8")


def _layered_write_seeds(*, project_dir: Path, seed_count: int) -> None:
    seeds_dir: Path = project_dir / "seeds"
    seeds_dir.mkdir()
    schema_entries: str = "\n".join(
        f"""  - name: seed_{index:05d}
    description: Test seed seed_{index:05d}.
    columns:
      - name: id
        type: INTEGER
      - name: label
        type: VARCHAR"""
        for index in range(seed_count)
    )
    (seeds_dir / "generated.yml").write_text(f"seeds:\n{schema_entries}\n", encoding="utf-8")
    for index in range(seed_count):
        (seeds_dir / f"seed_{index:05d}.csv").write_text(
            f"id,label\n{index},seed_{index:05d}\n",
            encoding="utf-8",
        )


def _layered_write_functions(*, project_dir: Path, function_count: int) -> None:
    functions_dir: Path = project_dir / "functions" / "sql"
    functions_dir.mkdir(parents=True)
    for index in range(function_count):
        (functions_dir / f"fn_{index:05d}.sql").write_text(
            """FUNCTION (description "Test function.",
  arguments (input_value DOUBLE),
  returns DOUBLE,
);

input_value + 1
""",
            encoding="utf-8",
        )


def _layered_write_macros(*, project_dir: Path, macro_count: int) -> None:
    macros_dir: Path = project_dir / "macros" / "python"
    macros_dir.mkdir(parents=True)
    for index in range(macro_count):
        composed_dependency: str = {
            True: (
                '\n\ndef base_offset(expression: str) -> str:\n    return f"({expression} + 1)"\n'
            ),
            False: "",
        }[index == 0]
        macro_body: str = {
            True: "    return base_offset(_offset(expression))",
            False: "    return _offset(expression)",
        }[index == 0]
        (macros_dir / f"macro_{index:05d}.py").write_text(
            f"""def _offset(expression: str) -> str:
    return f"({{expression}} + {index})"
{composed_dependency}


def macro_{index:05d}(expression: str) -> str:
{macro_body}
""",
            encoding="utf-8",
        )


def _keep_project_macros(*, project_dir: Path) -> None:
    """Leave generated macros in the project-wide macro directory."""


def _scope_single_folder_macros(*, project_dir: Path) -> None:
    """Move each macro beside its only consuming folder and drop unused macros."""

    consumer_folders: dict[str, set[str]] = {}
    for model_path in sorted((project_dir / "models").rglob("*.sql")):
        for call in _BENCHMARK_MACRO_CALL.findall(model_path.read_text(encoding="utf-8")):
            consumer_folders.setdefault(call, set()).add(model_path.parent.name)
    macros_dir: Path = project_dir / "macros" / "python"
    generated: set[str] = {path.stem for path in macros_dir.glob("macro_*.py")}
    for unused_stem in sorted(generated - set(consumer_folders)):
        (macros_dir / f"{unused_stem}.py").unlink()
    assert all(len(folders) == 1 for folders in consumer_folders.values())
    for stem, folders in sorted(consumer_folders.items()):
        (folder,) = folders
        scoped_dir: Path = project_dir / "models" / folder / "_sqlbuild" / "_macros" / "python"
        scoped_dir.mkdir(parents=True, exist_ok=True)
        (macros_dir / f"{stem}.py").rename(scoped_dir / f"{stem}.py")


def _layered_write_schemas(*, project_dir: Path) -> None:
    """Write the contract schema beside its only consumer, the first staging model."""

    schemas_dir: Path = (
        project_dir / "models" / _layered_model_folder(index=0) / "_sqlbuild" / "_schemas"
    )
    schemas_dir.mkdir(parents=True)
    (schemas_dir / "benchmark_row.sql").write_text(
        """SCHEMA (description "Test schema.",
  name benchmark_row,
  columns (
    id (type INTEGER, nullable false),
    amount (type DOUBLE),
    status (type VARCHAR),
  ),
);
""",
        encoding="utf-8",
    )


def _layered_write_models(
    *,
    project_dir: Path,
    model_count: int,
    source_count: int,
    seed_count: int,
    function_count: int,
    macro_count: int,
    audit_count: int,
) -> None:
    for index in range(model_count):
        folder: str = _layered_model_folder(index=index)
        model_dir: Path = project_dir / "models" / folder
        model_dir.mkdir(parents=True, exist_ok=True)
        sql: str = _layered_model_sql(
            index=index,
            model_count=model_count,
            source_count=source_count,
            audit_count=audit_count,
            seed_count=seed_count,
            function_count=function_count,
            macro_count=macro_count,
        )
        (model_dir / f"model_{index:05d}.sql").write_text(sql, encoding="utf-8")


def _layered_model_folder(*, index: int) -> str:
    layer_index: int = index % 10
    return {
        (True, True): "staging",
        (False, True): "intermediate",
        (False, False): "mart",
    }[(layer_index < 4, layer_index < 8)]


def _layered_is_base_model(*, index: int) -> bool:
    return index == 0 or (index >= _SPINE_DEPTH and (index - _SPINE_DEPTH) % _TEST_CHAIN_DEPTH == 0)


def _layered_model_header(*, index: int, audit_count: int) -> str:
    contract_header: str = """MODEL (
  model_schema benchmark_row,
  contract enforced,
  columns (
    id (audits [not_null]),
  ),
);"""
    audit_header: str = """MODEL (description "Test model.",
  columns (
    id (nullable false, audits [not_null]),
  ),
);"""
    return {
        (True, False): contract_header,
        (False, True): "MODEL (description 'Test model.');",
        (False, False): audit_header,
    }[(index == 0, index >= audit_count)]


def _layered_model_sql(
    *,
    index: int,
    model_count: int,
    source_count: int,
    seed_count: int,
    function_count: int,
    macro_count: int,
    audit_count: int,
) -> str:
    builders: dict[bool, Callable[[], str]] = {
        True: lambda: _layered_base_model_sql(
            index=index,
            model_count=model_count,
            source_count=source_count,
            audit_count=audit_count,
        ),
        False: lambda: _layered_dependent_model_sql(
            index=index,
            model_count=model_count,
            seed_count=seed_count,
            function_count=function_count,
            macro_count=macro_count,
            audit_count=audit_count,
        ),
    }
    return builders[_layered_is_base_model(index=index)]()


def _layered_base_model_sql(
    *, index: int, model_count: int, source_count: int, audit_count: int
) -> str:
    source_index: int = _layered_base_source_index(index=index, source_count=source_count)
    contract_query_sql: str = f"""{_layered_model_header(index=index, audit_count=audit_count)}

SELECT
  CAST(id AS INTEGER) AS id,
  CAST(amount + CAST(@@benchmark_revision AS INTEGER) AS DOUBLE) AS amount,
  CAST(status AS VARCHAR) AS status
FROM __source("source_{source_index:05d}")
"""
    regular_query_sql: str = f"""{_layered_model_header(index=index, audit_count=audit_count)}

SELECT
  id,
  amount + {_layered_generated_mapping_expression()}
    + CAST(@@benchmark_revision AS INTEGER) AS amount,
  status
FROM __source("source_{source_index:05d}")
"""
    query_sql: str = {True: contract_query_sql, False: regular_query_sql}[index == 0]
    return _layered_pad_model_sql(
        sql=query_sql,
        target_bytes=_layered_model_sql_size_target(index=index, model_count=model_count),
        index=index,
    )


def _layered_dependent_model_sql(
    *,
    index: int,
    model_count: int,
    seed_count: int,
    function_count: int,
    macro_count: int,
    audit_count: int,
) -> str:
    previous_name: str = f"model_{index - 1:05d}"
    macro_index: int = (index // _MACRO_INTERVAL) % macro_count
    id_expression: str = {
        True: f'@py.macro_{macro_index:05d}("previous.id")',
        False: f"previous.id + {index % 7}",
    }[index % _MACRO_INTERVAL == 0]
    generated_amount_expression: str = (
        f"previous.amount + {index % 11} + "
        f"{_layered_generated_mapping_expression(column='previous.id')} "
        "+ CAST(@@benchmark_revision AS INTEGER)"
    )
    function_index: int = index % function_count
    amount_expression: str = {
        True: f'__udf("fn_{function_index:05d}")(previous.amount)',
        False: generated_amount_expression,
    }[index % _FUNCTION_INTERVAL == 0]
    seed_index: int = index % seed_count
    join_sql: str = {
        True: f'\nLEFT JOIN __seed("seed_{seed_index:05d}") AS seed ON seed.id = previous.id',
        False: "",
    }[index >= _SEED_REFERENCE_START_INDEX and index % _SEED_INTERVAL == 0]
    query: str = f'''SELECT
  {id_expression} AS id,
  {amount_expression} AS amount,
  CASE WHEN previous.id % 2 = 0 THEN 'even' ELSE 'odd' END AS status
FROM __ref("{previous_name}") AS previous{join_sql}
'''
    direct_sql: str = f"{_layered_model_header(index=index, audit_count=audit_count)}\n\n{query}"
    with_sql: str = f"""{_layered_model_header(index=index, audit_count=audit_count)}

WITH transformed AS (
{query.rstrip()}
)
SELECT id, amount, status FROM transformed
"""
    nested_sql: str = f"""{_layered_model_header(index=index, audit_count=audit_count)}

SELECT id, amount, status
FROM (
{query.rstrip()}
) AS nested_query
"""
    model_sql: str = {
        (True, True): with_sql,
        (True, False): with_sql,
        (False, True): nested_sql,
        (False, False): direct_sql,
    }[
        (
            index % _TOP_LEVEL_WITH_INTERVAL == 0,
            index % _NESTED_QUERY_INTERVAL == 0,
        )
    ]
    return _layered_pad_model_sql(
        sql=model_sql,
        target_bytes=_layered_model_sql_size_target(index=index, model_count=model_count),
        index=index,
    )


def _semantic_write_models(
    *,
    project_dir: Path,
    model_count: int,
    source_count: int,
    seed_count: int,
    function_count: int,
    macro_count: int,
    audit_count: int,
    macro_call_interval: int = _MACRO_INTERVAL,
) -> None:
    for index in range(model_count):
        model_dir: Path = project_dir / "models" / _layered_model_folder(index=index)
        model_dir.mkdir(parents=True, exist_ok=True)
        column_count: int = _semantic_model_column_count(index=index, model_count=model_count)
        builders: dict[bool, Callable[[], str]] = {
            True: lambda index=index, column_count=column_count: _semantic_set_operation_model_sql(
                index=index,
                source_count=source_count,
                column_count=column_count,
                model_count=model_count,
                audit_count=audit_count,
            ),
            False: lambda index=index, column_count=column_count: _semantic_regular_model_sql(
                index=index,
                model_count=model_count,
                source_count=source_count,
                seed_count=seed_count,
                function_count=function_count,
                macro_count=macro_count,
                audit_count=audit_count,
                column_count=column_count,
                macro_call_interval=macro_call_interval,
            ),
        }
        sql: str = builders[index == _SEMANTIC_SET_OPERATION_MODEL_INDEX]()
        (model_dir / f"model_{index:05d}.sql").write_text(sql, encoding="utf-8")


def _semantic_model_column_count(*, index: int, model_count: int) -> int:
    quantile: float = (index + 1) / model_count
    upper_quantiles: tuple[float, ...] = tuple(item[0] for item in _SEMANTIC_COLUMN_COUNT_PROFILE)
    upper_index: int = bisect_left(upper_quantiles, quantile)
    lower_quantile, lower_count = _SEMANTIC_COLUMN_COUNT_PROFILE[upper_index - 1]
    upper_quantile, upper_count = _SEMANTIC_COLUMN_COUNT_PROFILE[upper_index]
    position: float = (quantile - lower_quantile) / (upper_quantile - lower_quantile)
    profile_count: int = max(3, round(lower_count + position * (upper_count - lower_count)))
    return {
        False: profile_count,
        True: 270,
    }[index == _SEMANTIC_SET_OPERATION_MODEL_INDEX]


def _semantic_model_header(
    *, index: int, model_count: int, audit_count: int, column_count: int
) -> str:
    base_audit_count, extra_audit_count = divmod(audit_count, model_count)
    model_audit_count: int = base_audit_count + int(index < extra_audit_count)
    base_columns: list[tuple[str, str]] = [
        ("id", "INTEGER"),
        ("amount", "DOUBLE"),
        ("status", "VARCHAR"),
    ]
    generated_columns: list[tuple[str, str]] = [
        (f"metric_{column_index:04d}", "DOUBLE")
        for column_index in range(column_count - len(base_columns))
    ]
    columns: list[tuple[str, str]] = [*base_columns, *generated_columns]
    declarations: list[str] = []
    for column_index, (name, data_type) in enumerate(columns):
        audit_sql: str = {False: "", True: ", audits [not_null]"}[column_index < model_audit_count]
        nullable_sql: str = {False: "", True: ", nullable false"}[column_index == 0]
        declarations.append(f"    {name} (type {data_type}{nullable_sql}{audit_sql})")
    return (
        'MODEL (\n  description "Generated benchmark model.",\n  columns (\n'
        + ",\n".join(declarations)
        + "\n  ),\n);"
    )


def _semantic_regular_model_sql(
    *,
    index: int,
    model_count: int,
    source_count: int,
    seed_count: int,
    function_count: int,
    macro_count: int,
    audit_count: int,
    column_count: int,
    macro_call_interval: int = _MACRO_INTERVAL,
) -> str:
    header: str = _semantic_model_header(
        index=index,
        model_count=model_count,
        audit_count=audit_count,
        column_count=column_count,
    )
    source_index: int = _layered_base_source_index(index=index, source_count=source_count)
    relation_sql: str = {
        True: f'__source("source_{source_index:05d}")',
        False: f'__ref("model_{index - 1:05d}")',
    }[_layered_is_base_model(index=index)]
    metric_expressions: str = "".join(
        f",\n  CAST(COALESCE(CASE WHEN input.id % {column_index % 11 + 2} = 0 "
        f"THEN amount + {column_index} WHEN status = 'priority' "
        f"THEN amount * {column_index % 7 + 1} ELSE amount - {column_index} END, 0) AS DOUBLE) "
        f"AS metric_{column_index:04d}"
        for column_index in range(column_count - 3)
    )
    seed_index: int = index % seed_count
    seed_join_sql: str = {
        True: f'\nLEFT JOIN __seed("seed_{seed_index:05d}") AS seed ON seed.id = input.id',
        False: "",
    }[index >= _SEED_REFERENCE_START_INDEX and index % _SEED_INTERVAL == 0]
    function_index: int = index % function_count
    amount_expression: str = {
        True: f'__udf("fn_{function_index:05d}")(amount)',
        False: "amount + CAST(@@benchmark_revision AS INTEGER)",
    }[index % _FUNCTION_INTERVAL == 0]
    macro_index: int = (index // macro_call_interval) % macro_count
    id_expression: str = {
        True: f'@py.macro_{macro_index:05d}("input.id")',
        False: "input.id",
    }[index % macro_call_interval == 0]
    status_expression: str = (
        "CAST(CASE WHEN input.id % 2 = 0 THEN 'even' ELSE 'odd' END AS VARCHAR)"
    )
    direct_sql: str = f"""SELECT
  CAST({id_expression} AS INTEGER) AS id,
  CAST({amount_expression} AS DOUBLE) AS amount,
  {status_expression} AS status{metric_expressions}
FROM {relation_sql} AS input{seed_join_sql}
"""
    with_sql: str = f"WITH transformed AS (\n{direct_sql.rstrip()}\n)\nSELECT * FROM transformed\n"
    nested_sql: str = f"SELECT * FROM (\n{direct_sql.rstrip()}\n) AS nested_query\n"
    query_sql: str = {
        (True, True): with_sql,
        (True, False): with_sql,
        (False, True): nested_sql,
        (False, False): direct_sql,
    }[
        (
            index % _TOP_LEVEL_WITH_INTERVAL == 0,
            index % _NESTED_QUERY_INTERVAL == 0,
        )
    ]
    return f"{header}\n\n{query_sql}"


def _semantic_set_operation_model_sql(
    *,
    index: int,
    source_count: int,
    column_count: int,
    model_count: int,
    audit_count: int,
) -> str:
    header: str = _semantic_model_header(
        index=index,
        model_count=model_count,
        audit_count=audit_count,
        column_count=column_count,
    )
    source_index: int = _layered_base_source_index(index=index, source_count=source_count)
    branches: str = "\nUNION ALL\n".join(
        f"SELECT {branch % 5 + 1} AS bucket, {branch * 10} AS offset_seconds, "
        f"CAST({branch % 13} AS DOUBLE) AS adjustment"
        for branch in range(_SEMANTIC_SET_OPERATION_BRANCH_COUNT)
    )
    metrics: str = "".join(
        f",\n  CAST(MAX(CASE WHEN bucket = {metric % 5 + 1} "
        f"AND offset_seconds = {metric * 10} THEN amount + adjustment END) AS DOUBLE) "
        f"AS metric_{metric:04d}"
        for metric in range(column_count - 3)
    )
    return f"""{header}

WITH offset_grid AS (
{branches}
), measurements AS (
  SELECT source.id, source.amount, source.status, grid.bucket, grid.offset_seconds, grid.adjustment
  FROM __source("source_{source_index:05d}") AS source
  CROSS JOIN offset_grid AS grid
)
SELECT
  CAST(MAX(id) AS INTEGER) AS id,
  CAST(MAX(amount) AS DOUBLE) AS amount,
  CAST(MAX(status) AS VARCHAR) AS status{metrics}
FROM measurements
"""


SHARED_DIAMOND_HUB: str = "shared_orders_hub"
SHARED_DIAMOND_ROLLUP: str = "shared_orders_rollup"
_SHARED_DIAMOND_FOLDER: str = "intermediate/shared_orders"
_SHARED_DIAMOND_UPSTREAM_MODEL: str = f"model_{_SPINE_DEPTH - 1:05d}"
_SHARED_DIAMOND_HEADER: str = """MODEL (description "Test model.",
  columns (
    id (type INTEGER, nullable false),
    amount (type DOUBLE),
    status (type VARCHAR)
  ),
);"""


def shared_diamond_model_name(*, layer: int, slot: int) -> str:
    """Return the lattice model name for one layer and slot."""

    return f"shared_orders_l{layer:02d}_s{slot:02d}"


def write_shared_diamond_models(*, project_dir: Path, layer_count: int, width: int) -> None:
    """Write a shared-dependency lattice with repeated fan-out and fan-in.

    The hub reads the end of the spine and fans out to `width` first-layer models. Every later
    slot joins the two neighbouring slots of the previous layer, so each model is reachable along
    exponentially many paths, and the rollup fans the final layer back in.
    """

    model_dir: Path = project_dir / "models" / _SHARED_DIAMOND_FOLDER
    model_dir.mkdir(parents=True)
    files: dict[str, str] = {
        SHARED_DIAMOND_HUB: _shared_diamond_sql(inputs=(_SHARED_DIAMOND_UPSTREAM_MODEL,)),
        SHARED_DIAMOND_ROLLUP: _shared_diamond_sql(
            inputs=tuple(
                shared_diamond_model_name(layer=layer_count - 1, slot=slot) for slot in range(width)
            )
        ),
    }
    for layer in range(layer_count):
        for slot in range(width):
            files[shared_diamond_model_name(layer=layer, slot=slot)] = _shared_diamond_sql(
                inputs=_shared_diamond_inputs(layer=layer, slot=slot, width=width)
            )
    for name, sql in files.items():
        (model_dir / f"{name}.sql").write_text(sql, encoding="utf-8")


def _shared_diamond_inputs(*, layer: int, slot: int, width: int) -> tuple[str, ...]:
    builders: dict[bool, Callable[[], tuple[str, ...]]] = {
        True: lambda: (SHARED_DIAMOND_HUB,),
        False: lambda: (
            shared_diamond_model_name(layer=layer - 1, slot=slot),
            shared_diamond_model_name(layer=layer - 1, slot=(slot + 1) % width),
        ),
    }
    return builders[layer == 0]()


def _shared_diamond_sql(*, inputs: tuple[str, ...]) -> str:
    amount: str = " + ".join(f"input_{index:02d}.amount" for index in range(len(inputs)))
    joins: str = "".join(
        f'\nJOIN __ref("{name}") AS input_{index:02d} ON input_{index:02d}.id = input_00.id'
        for index, name in enumerate(inputs[1:], start=1)
    )
    return f"""{_SHARED_DIAMOND_HEADER}

SELECT
  CAST(input_00.id AS INTEGER) AS id,
  CAST({amount} AS DOUBLE) AS amount,
  CAST(input_00.status AS VARCHAR) AS status
FROM __ref("{inputs[0]}") AS input_00{joins}
"""


def _layered_write_tests(
    *,
    project_dir: Path,
    model_count: int,
    test_count: int,
    source_count: int,
    seed_count: int,
    semantic_fixture_scale: bool = False,
) -> None:
    tests_dir: Path = project_dir / "tests" / "unit"
    tests_dir.mkdir(parents=True)
    tests_per_file: int = 4
    cases_per_target: int = 5
    repeated_target_count: int = (test_count + cases_per_target - 1) // cases_per_target
    for file_index in range((test_count + tests_per_file - 1) // tests_per_file):
        first_test_index: int = file_index * tests_per_file
        blocks: str = "\n".join(
            _layered_test_block(
                test_index=test_index,
                model_count=model_count,
                source_count=source_count,
                repeated_target_count=repeated_target_count,
                seed_count=seed_count,
                semantic_fixture_scale=semantic_fixture_scale,
            )
            for test_index in range(
                first_test_index,
                min(test_count, first_test_index + tests_per_file),
            )
        )
        (tests_dir / f"test_group_{file_index:05d}.sql").write_text(blocks, encoding="utf-8")


def _layered_test_block(
    *,
    test_index: int,
    model_count: int,
    source_count: int,
    repeated_target_count: int,
    seed_count: int,
    semantic_fixture_scale: bool = False,
) -> str:
    group_count: int = (model_count - _SPINE_DEPTH) // _TEST_CHAIN_DEPTH
    representative_group_count: int = {
        True: group_count // 3,
        False: (group_count * 2) // 3,
    }[semantic_fixture_scale]
    group_index: int = ((test_index // 5) * representative_group_count) // repeated_target_count
    base_index: int = _SPINE_DEPTH + group_index * _TEST_CHAIN_DEPTH
    target_index: int = base_index + _TEST_CHAIN_DEPTH - 1
    source_index: int = _layered_base_source_index(index=base_index, source_count=source_count)
    fixture_row_count: int = {
        True: 5 + (test_index % 5) * 5,
        False: 40 + (test_index % 5) * 40,
    }[semantic_fixture_scale]
    first_seed_index: int = max(
        _SEED_REFERENCE_START_INDEX, base_index + int(not semantic_fixture_scale)
    )
    first_seed_index = ((first_seed_index + _SEED_INTERVAL - 1) // _SEED_INTERVAL) * _SEED_INTERVAL
    seed_indexes: set[int] = {
        index % seed_count for index in range(first_seed_index, target_index + 1, _SEED_INTERVAL)
    }
    seed_fixtures: str = "".join(
        f"__seed__seed_{index:05d} AS (SELECT 1 AS id),\n" for index in sorted(seed_indexes)
    )
    return _layered_test_sql(
        test_index=test_index,
        source_index=source_index,
        target_index=target_index,
        fixture_row_count=fixture_row_count,
        include_assertion=test_index % 7 == 0,
        seed_fixtures=seed_fixtures,
    )


def _layered_test_sql(
    *,
    test_index: int,
    source_index: int,
    target_index: int,
    fixture_row_count: int,
    include_assertion: bool,
    seed_fixtures: str,
) -> str:
    fixture_rows: str = " UNION ALL\n".join(
        f"  SELECT {row} AS id, CAST({row} AS DOUBLE) AS amount, 'source' AS status"
        for row in range(fixture_row_count)
    )
    assertion_sql: str = f""",
__assert__non_negative_{target_index:05d} AS (
  SELECT * FROM __ref("model_{target_index:05d}") WHERE amount < 0
)"""
    assertion: str = {True: assertion_sql, False: ""}[include_assertion]
    return f"""TEST (name "layered_production_case_{test_index:05d}");

WITH
{seed_fixtures}__source__source_{source_index:05d} AS (
{fixture_rows}
),
__expected__model_{target_index:05d} AS (
  SELECT 1 AS id, CAST(1 AS DOUBLE) AS amount, 'odd' AS status
){assertion}
SELECT 1
"""


def _layered_base_source_index(*, index: int, source_count: int) -> int:
    base_ordinal: int = {
        True: 0,
        False: 1 + (index - _SPINE_DEPTH) // _TEST_CHAIN_DEPTH,
    }[index == 0]
    return base_ordinal % source_count


def _layered_model_sql_size_target(*, index: int, model_count: int) -> int:
    quantile: float = (index + 1) / model_count
    upper_quantiles: tuple[float, ...] = tuple(item[0] for item in _SQL_SIZE_PROFILE)
    upper_index: int = bisect_left(upper_quantiles, quantile)
    lower_quantile, lower_size = _SQL_SIZE_PROFILE[upper_index - 1]
    upper_quantile, upper_size = _SQL_SIZE_PROFILE[upper_index]
    position: float = (quantile - lower_quantile) / (upper_quantile - lower_quantile)
    return round(lower_size + position * (upper_size - lower_size))


def _layered_generated_mapping_expression(*, column: str = "id") -> str:
    clauses: str = "".join(
        f"WHEN {column} = {value:05d} THEN {value % 13:02d} " for value in range(45)
    )
    return f"CASE {clauses} ELSE 0 END"


def _layered_pad_model_sql(*, sql: str, target_bytes: int, index: int) -> str:
    missing_bytes: int = max(0, target_bytes - len(sql.encode()))
    line_template: str = "-- generated field 00000 maps source_metric to output_metric_00000\n"
    line_count: int = (missing_bytes + len(line_template) - 1) // len(line_template)
    comments: str = "".join(
        f"-- generated field {line:05d} maps source_metric to output_metric_{index:05d}\n"
        for line in range(line_count)
    )
    return f"{sql.rstrip()}\n{comments}"


def build_rule_gated_test_project_files(*, filler_model_count: int) -> dict[str, str]:
    """Build a project with one rule error and one SQL test that lacks a source mock."""

    files: dict[str, str] = {
        "sqlbuild_project.toml": (
            'name = "rule_gated_orders"\nadapter = "duckdb"\n\n[rules]\nselect = ["SQBRMODEL102"]\n'
        ),
        "sources/raw.yml": (
            "sources:\n"
            "  - name: raw_orders\n    description: Test source raw_orders.\n"
            "    schema: main\n    table: raw_orders\n"
            "  - name: raw_refunds\n    description: Test source raw_refunds.\n"
            "    schema: main\n    table: raw_refunds\n"
        ),
        "models/orders.sql": (
            "MODEL (description 'Test model orders.', materialized view);\n\n"
            'SELECT o.order_id, r.refund_id FROM __source("raw_orders") AS o\n'
            'LEFT JOIN __source("raw_refunds") AS r ON r.order_id = o.order_id\n'
        ),
        "models/bad_star.sql": (
            'MODEL (description "Test model bad_star.", '
            'materialized view);\n\nSELECT * FROM __source("raw_orders")\n'
        ),
        "tests/unit/orders_case.sql": (
            'TEST (name "orders_case");\n'
            "WITH\n"
            "__source__raw_orders AS (SELECT 1 AS order_id),\n"
            "__expected__orders AS (SELECT 1 AS order_id)\n"
            "SELECT 1\n"
        ),
    }
    for index in range(filler_model_count):
        files[f"models/filler/filler_{index:03d}.sql"] = (
            "MODEL (description 'Test model.', "
            f"materialized view);\n\nSELECT {index} AS filler_id\n"
        )
    return files


def build_empty_input_test_project_files(
    *, select: tuple[str, ...], allowed_tests_toml: str
) -> dict[str, str]:
    """Build an orders project mixing empty-input-only filler with real SQL tests."""

    selected: str = ", ".join(f'"{code}"' for code in select)
    empty_orders: str = (
        "__source__raw_orders AS (\n"
        "  SELECT CAST(NULL AS INTEGER) AS order_id, CAST(NULL AS INTEGER) AS amount\n"
        "  WHERE FALSE\n"
        ")"
    )
    return {
        "sqlbuild_project.toml": (
            'name = "empty_input_orders"\nadapter = "duckdb"\n\n'
            f"[rules]\nselect = [{selected}]\n\n"
            "[rules.thresholds]\nmin_tests_per_model = 1\n\n"
            f"[rules.rule_options.SQBRTEST203]\nallowed_tests = {allowed_tests_toml}\n"
        ),
        "sources/raw.yml": (
            "sources:\n  - name: raw_orders\n    description: Test source raw_orders.\n"
            "    schema: main\n    table: raw_orders\n"
        ),
        "models/orders.sql": (
            "MODEL (description 'Test model orders.', materialized view);\n\n"
            "SELECT order_id, amount * 2 AS doubled_amount\n"
            'FROM __source("raw_orders")\nWHERE amount > 0\n'
        ),
        "models/large_orders.sql": (
            "MODEL (description 'Test model large_orders.', materialized view);\n\n"
            'SELECT order_id FROM __source("raw_orders") WHERE amount > 100\n'
        ),
        "models/customers.sql": (
            "MODEL (description 'Test model customers.', materialized view);\n\n"
            'SELECT order_id AS customer_id FROM __source("raw_orders") WHERE amount > 10\n'
        ),
        "models/order_summary.sql": (
            "MODEL (description 'Test model order_summary.', materialized view);\n\n"
            "SELECT COUNT(*) AS order_count, COALESCE(SUM(amount), 0) AS total_amount\n"
            'FROM __source("raw_orders")\n'
        ),
        "tests/unit/test_orders__empty_inputs_produce_no_rows.sql": (
            'TEST (name "orders__empty_inputs_produce_no_rows");\n\n'
            f"WITH {empty_orders}, __assert__empty_inputs_produce_no_rows AS (\n"
            '  SELECT 1 AS unexpected_row FROM __ref("orders")\n'
            ")\nSELECT 1\n"
        ),
        "tests/unit/test_large_orders__empty_inputs_produce_no_rows.sql": (
            'TEST (name "large_orders__empty_inputs_produce_no_rows");\n\n'
            f"WITH {empty_orders}, __expected__large_orders AS (\n"
            "  SELECT * FROM __EMPTY_FIXTURE()\n"
            ")\nSELECT 1\n"
        ),
        "tests/unit/test_large_orders__keeps_orders_over_threshold.sql": (
            'TEST (name "large_orders__keeps_orders_over_threshold");\n\n'
            "WITH __source__raw_orders AS (\n"
            "  SELECT 1 AS order_id, 150 AS amount UNION ALL SELECT 2 AS order_id, 20 AS amount\n"
            "), __expected__large_orders AS (\n"
            "  SELECT 1 AS order_id\n"
            ")\nSELECT 1\n"
        ),
        "tests/unit/test_customers__reviewed_empty_input.sql": (
            'TEST (name "customers__reviewed_empty_input");\n\n'
            f"WITH {empty_orders}, __assert__no_customers AS (\n"
            '  SELECT customer_id FROM __ref("customers")\n'
            ")\nSELECT 1\n"
        ),
        "tests/unit/test_order_summary__empty_inputs_return_zero_row.sql": (
            'TEST (name "order_summary__empty_inputs_return_zero_row");\n\n'
            f"WITH {empty_orders}, __expected__order_summary AS (\n"
            "  SELECT 0 AS order_count, 0 AS total_amount\n"
            ")\nSELECT 1\n"
        ),
    }


class BuiltBenchmark(NamedTuple):
    project_dir: Path
    build: InspectionCommandMeasurement


BUILD_BENCHMARK_MODEL_COUNT: int = 1_000
_BUILD_BENCHMARK_CONNECTION: str = '\n[connection]\ndatabase = "benchmark.duckdb"\n'
_BUILD_BENCHMARK_EDIT: tuple[str, str] = ("amount + 0 WHEN", "amount + 100 WHEN")


def write_build_benchmark_project(
    *, project_dir: Path, model_count: int = BUILD_BENCHMARK_MODEL_COUNT
) -> None:
    """Write the build benchmark without tests or audits, scaled to model_count models."""

    scale: float = model_count / BUILD_BENCHMARK_MODEL_COUNT
    write_semantic_compile_project(
        project_dir=project_dir,
        model_count=model_count,
        source_count=max(1, round(238 * scale)),
        seed_count=max(1, round(47 * scale)),
        function_count=max(1, round(24 * scale)),
        macro_count=max(1, round(12 * scale)),
        test_count=0,
        audit_count=0,
    )
    config: Path = project_dir / "sqlbuild_project.toml"
    config.write_text(
        config.read_text(encoding="utf-8") + _BUILD_BENCHMARK_CONNECTION, encoding="utf-8"
    )


def prepare_build_benchmark_project(*, project_dir: Path) -> None:
    """Write a 1,000-model benchmark without tests or audits and warm its compile cache."""

    write_build_benchmark_project(project_dir=project_dir)
    _ = run_fresh_process_inspection_command(
        project_dir=project_dir,
        label="build-benchmark-compile",
        sqb_args=("compile", "--json"),
        expected_max_wall_seconds=60.0,
    )


def change_benchmark_models(
    *,
    project_dir: Path,
    edited_models: tuple[str, ...],
    renamed_models: tuple[tuple[str, str], ...],
) -> None:
    """Change the query of each edited model and rename each renamed model file."""

    name: str
    for name in edited_models:
        _replace_benchmark_text(
            path=_benchmark_model_path(project_dir=project_dir, name=name),
            old=_BUILD_BENCHMARK_EDIT[0],
            new=_BUILD_BENCHMARK_EDIT[1],
        )
    old_name: str
    new_name: str
    for old_name, new_name in renamed_models:
        path: Path = _benchmark_model_path(project_dir=project_dir, name=old_name)
        _ = path.rename(path.with_name(f"{new_name}.sql"))


def _benchmark_model_path(*, project_dir: Path, name: str) -> Path:
    return next((project_dir / "models").rglob(f"{name}.sql"))


def plan_reasons_and_migrations(
    payload: object,
) -> tuple[tuple[str, ...], tuple[tuple[str, str, str], ...]]:
    """Return query-changed model names and (origin, destination, decision) migrations."""

    plan: dict[str, Any] = cast(dict[str, Any], payload)
    return (
        tuple(sorted(model["name"] for model in filter(_query_changed, plan["models"]))),
        tuple(
            sorted(
                (migration["origin_model"], migration["model"], migration["decision"])
                for migration in plan["migrations"]
            )
        ),
    )


def _query_changed(model: dict[str, Any]) -> bool:
    return model["reason"] == "query_changed"


def write_relation_stub_project(
    *, tmp_path: Path, stub_cte_name: str, selected_column: str
) -> Path:
    """Write a project whose middle model authors a CTE named like a relation stub."""

    return prepare_inline_project(
        tmp_path=tmp_path,
        project_name="relation_stub_names",
        repo_files={
            "sqlbuild_project.toml": (
                'name = "relation_stub_names"\n'
                'adapter = "duckdb"\n'
                'default_target = "dev"\n\n'
                "[connection]\n"
                'database = "orders.duckdb"\n\n'
                "[targets.dev]\n"
                'schema = "main"\n\n'
                "[[rules.rule_ignores]]\n"
                'rules = ["SQLCLEAN005"]\n'
                'paths = ["models/order_status.sql"]\n'
                'reason = "The stub-named CTE is left unread on purpose."\n'
            ),
            "models/orders.sql": (
                "MODEL (description 'Test model orders.', "
                "materialized table);\n\nSELECT CAST(7 AS INTEGER) AS order_id\n"
            ),
            "models/order_status.sql": (
                "MODEL (description 'Test model order_status.', materialized table);\n\n"
                f"WITH {stub_cte_name} AS (\n"
                "  SELECT 'pending' AS order_id, 'pending' AS ghost_status\n"
                ")\n"
                f'SELECT {selected_column} FROM __ref("orders")\n'
            ),
            "models/next_order.sql": (
                "MODEL (description 'Test model next_order.',\n"
                "  materialized table,\n"
                "  contract enforced,\n"
                "  columns (order_id (type INTEGER), next_order_id (type INTEGER)),\n"
                ");\n\n"
                "SELECT order_id, order_id + 1 AS next_order_id\n"
                'FROM __ref("order_status")\n'
            ),
        },
    )


COMPILE_CACHE_REGION_ENV_VAR: str = "SQB_CACHE_INVALIDATION_REGION"
_COMPILE_CACHE_DIAGNOSTIC_PATTERN: re.Pattern[str] = re.compile(
    r"^(?:error|warning)\[.*$", re.MULTILINE
)
_COMPILE_CACHE_EXTRA_PROJECT_FILES: dict[str, str] = {
    "models/marts/_sqlbuild/_enums/order_channel.sql": (
        'ENUM (\n  name order_channel,\n  members (WEB "web", PARTNER "partner"),\n);\n'
    ),
    "models/marts/_sqlbuild/_constants/min_quantity.sql": (
        "CONSTANT (name min_quantity, value 1);\n"
    ),
    "models/marts/channel_orders.sql": (
        "MODEL (description 'Test model channel_orders.',\n"
        "  materialized table,\n"
        "  columns (\n"
        "    order_id (nullable false, audits [not_null]),\n"
        "  ),\n"
        ");\n\n"
        "SELECT\n"
        "  o.order_id,\n"
        '  @enum("order_channel").WEB AS order_channel,\n'
        '  o.quantity >= @const("min_quantity") AS meets_minimum,\n'
        "  CAST(@@quantity_multiplier AS INTEGER) * o.quantity AS scaled_quantity,\n"
        "  '@@ENV:" + COMPILE_CACHE_REGION_ENV_VAR + "' AS region\n"
        'FROM __ref("stg_orders") o\n'
    ),
    "tests/unit/test_channel_orders.sql": (
        "TEST ();\n\n"
        "WITH\n"
        "__ref__stg_orders AS (\n"
        "  SELECT 1 AS order_id, 2 AS quantity\n"
        "),\n"
        "__expected__channel_orders AS (\n"
        "  SELECT 1 AS order_id, 'web' AS order_channel, TRUE AS meets_minimum\n"
        ")\n"
        "SELECT 1\n"
    ),
    "tests/unit/test_channel_blocks.sql": (
        'TEST (name "channel_orders_first");\n\n'
        "WITH\n"
        "__ref__stg_orders AS (\n"
        "  SELECT 1 AS order_id, 2 AS quantity\n"
        "),\n"
        "__expected__channel_orders AS (\n"
        "  SELECT 1 AS order_id, 'web' AS order_channel\n"
        ")\n"
        "SELECT 1\n\n"
        'TEST (name "channel_orders_second");\n\n'
        "WITH\n"
        "__ref__stg_orders AS (\n"
        "  SELECT 2 AS order_id, 5 AS quantity\n"
        "),\n"
        "__expected__channel_orders AS (\n"
        "  SELECT 2 AS order_id, 'web' AS order_channel\n"
        ")\n"
        "SELECT 1\n"
    ),
}


class CompileCacheOutcome(NamedTuple):
    """Comparable result of one fresh-process compile."""

    returncode: int
    diagnostics: tuple[str, ...]
    fingerprint: str
    analysis_cache_hits: int
    analysis_cache_misses: int


def run_installed_sqb(
    *, project_dir: Path, args: tuple[str, ...], env: dict[str, str]
) -> subprocess.CompletedProcess[str]:
    """Run the installed sqb entrypoint in a fresh process with extra environment values."""

    return subprocess.run(
        [
            str(Path(sys.executable).with_name("sqb")),
            "--project-dir",
            str(project_dir),
            "--no-color",
            *args,
        ],
        capture_output=True,
        text=True,
        env={**os.environ, **env},
        check=False,
    )


def compile_cache_outcome(
    *, project_dir: Path, env: dict[str, str], compile_args: tuple[str, ...] = ()
) -> CompileCacheOutcome:
    """Compile in a fresh process and return its semantic fingerprint and analysis-cache counts."""

    result: subprocess.CompletedProcess[str] = run_installed_sqb(
        project_dir=project_dir, args=("compile", "--json", *compile_args), env=env
    )
    values: dict[str, object] = cast(dict[str, object], json.loads(result.stdout or "{}"))
    timings: dict[str, int] = cast(dict[str, int], values.get("compile_timings", {}))
    return CompileCacheOutcome(
        returncode=result.returncode,
        diagnostics=tuple(_COMPILE_CACHE_DIAGNOSTIC_PATTERN.findall(result.stderr)),
        fingerprint=semantic_compile_fingerprint(
            payload=values, compiled_dir=project_dir / "target" / "compiled"
        ),
        analysis_cache_hits=timings.get("analysis_batch_cache_hits", 0)
        + timings.get("analysis_entry_cache_hits", 0),
        analysis_cache_misses=timings.get("analysis_cache_misses", 0),
    )


def replace_project_text(project_dir: Path, relative_path: str, old: str, new: str) -> None:
    """Replace the first occurrence of authored text, failing when it is absent."""

    path: Path = project_dir / relative_path
    contents: str = path.read_text(encoding="utf-8")
    assert old in contents, f"{old!r} not found in {relative_path}"
    path.write_text(contents.replace(old, new, 1), encoding="utf-8")


def write_project_file(project_dir: Path, relative_path: str, contents: str) -> None:
    """Write one authored project file, creating parent folders."""

    path: Path = project_dir / relative_path
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(contents, encoding="utf-8")


def move_project_file(project_dir: Path, source: str, destination: str) -> None:
    """Move one authored project file into a possibly new folder."""

    (project_dir / destination).parent.mkdir(parents=True, exist_ok=True)
    (project_dir / source).rename(project_dir / destination)


def prepare_compile_cache_invalidation_project(*, project_dir: Path) -> None:
    """Create a playground project covering every cached compile input kind."""

    result: subprocess.CompletedProcess[str] = run_installed_sqb(
        project_dir=project_dir.parent, args=("playground", str(project_dir)), env={}
    )
    assert result.returncode == 0, result.stdout + result.stderr
    shutil.rmtree(project_dir / "target", ignore_errors=True)
    for relative_path, contents in _COMPILE_CACHE_EXTRA_PROJECT_FILES.items():
        write_project_file(project_dir, relative_path, contents)
    replace_project_text(
        project_dir,
        "sqlbuild_project.toml",
        "[settings]",
        '[vars]\nquantity_multiplier = "2"\n\n[settings]',
    )


RESOURCE_SQL_MODELS: str = "models/staging"
RESOURCE_SQL_ORDERS: str = f"{RESOURCE_SQL_MODELS}/orders.sql"
RESOURCE_SQL_SINGULAR_AUDIT: str = (
    f"{RESOURCE_SQL_MODELS}/_sqlbuild/audits/singular/orders_have_customers.sql"
)
RESOURCE_SQL_GENERIC_AUDIT: str = (
    f"{RESOURCE_SQL_MODELS}/_sqlbuild/_audits/generic/status_is_known.sql"
)
RESOURCE_SQL_RECENT_ROWS_AUDIT: str = (
    f"{RESOURCE_SQL_MODELS}/_sqlbuild/_audits/generic/recent_rows.sql"
)
RESOURCE_SQL_TEST: str = "tests/unit/test_orders.sql"
RESOURCE_SQL_NAMED_HOOK: str = f"{RESOURCE_SQL_MODELS}/_sqlbuild/_hooks/sql/record_orders.sql"
_RESOURCE_SQL_BASE_FILES: dict[str, str] = {
    "sqlbuild_project.toml": (
        'name = "orders"\nadapter = "duckdb"\n\n[connection]\ndatabase = "warehouse.duckdb"\n'
    ),
    f"{RESOURCE_SQL_MODELS}/customers.sql": (
        'MODEL (description "Customers per order");\n'
        "SELECT CAST(1 AS INTEGER) AS order_id, CAST(7 AS INTEGER) AS customer_id\n"
    ),
}


def resource_sql_orders_model(options: str = "") -> tuple[str, str]:
    """Return the orders model file whose header carries the given extra options."""

    return (
        RESOURCE_SQL_ORDERS,
        f'MODEL (description "Orders with their status"{options});\n'
        'WITH customers AS (SELECT * FROM __ref("customers"))\n'
        "SELECT CAST(c.order_id AS INTEGER) AS order_id, CAST('open' AS VARCHAR) AS status\n"
        "FROM customers AS c\n",
    )


def resource_sql_project_files(files: tuple[tuple[str, str], ...]) -> dict[str, str]:
    """Return the shared orders project plus case-specific files."""

    return {**_RESOURCE_SQL_BASE_FILES, **dict(files)}


REQUIRE_SQL_ANALYSIS_PROJECT: str = (
    'name = "orders"\nadapter = "duckdb"\n\n[settings]\nrequire_sql_analysis = true\n'
)
OPTIONAL_SQL_ANALYSIS_PROJECT: str = 'name = "orders"\nadapter = "duckdb"\n'
PARSEABLE_OPT_OUT_MODEL: str = (
    'MODEL (\n  description "Order flags",\n  sql_analysis false\n);\n'
    "SELECT 1 = 'pending' AS is_pending, 2 = 'shipped' AS is_shipped\n"
)
UNPARSEABLE_OPT_OUT_MODEL: str = (
    'MODEL (description "Order lookup", sql_analysis false);\nSELECT order_id FROM orders WHERE\n'
)
PATH_DEFAULT_MODEL: str = (
    'MODEL (description "Order epochs");\n'
    "SELECT CAST(TIMESTAMP '2026-04-01' AS INTEGER) AS ordered_epoch\n"
)
OPT_OUT_HELP: str = (
    "= help: to allow `sql_analysis false` on any model, SQL test or audit, set this in "
    "sqlbuild_project.toml:\n"
    "            [settings]\n"
    "            require_sql_analysis = false"
)


def require_sql_analysis_output(
    result: subprocess.CompletedProcess[str],
) -> tuple[tuple[tuple[str, str | None, int | None], ...], str]:
    """Return `(code, path, line)` per JSON diagnostic and all JSON help, note and stderr text.

    A compile that stops on a raised error reports it without a path or line.
    """

    payload: dict[str, Any] = json.loads(result.stdout or '{"diagnostics": []}')
    diagnostics: list[dict[str, Any]] = payload["diagnostics"]
    parts: list[str] = [result.stderr]
    for item in diagnostics:
        parts.append(str(item.get("help")))
        parts.extend(item.get("notes", ()))
    text: str = "\n".join(parts)
    return (
        tuple((item["code"], item.get("path"), item.get("line")) for item in diagnostics),
        text,
    )


_INDENTED_SET_OPERATION_PATTERN: re.Pattern[str] = re.compile(
    r"^[ \t]+(?:UNION|EXCEPT|INTERSECT)\b.*$", re.MULTILINE
)


class SetOperationCompileResult(NamedTuple):
    exit_code: int
    diagnostics: tuple[tuple[str, str], ...]
    column_counts: dict[str, int]
    output: str


class SetOperationLifecycleResult(NamedTuple):
    compiled: SetOperationCompileResult
    build: subprocess.CompletedProcess[str]
    relation_shapes: dict[str, tuple[int, int]]
    rules: subprocess.CompletedProcess[str]
    format: subprocess.CompletedProcess[str]
    indented_set_operation_lines: tuple[str, ...]
    recompiled: SetOperationCompileResult


def write_set_operation_project(
    *, project_dir: Path, models: tuple[SetOperationModel, ...], adapter: str = "duckdb"
) -> Path:
    """Write a project with one view per model and return its DuckDB database path.

    Other adapters compile offline against a default database and never connect.
    """

    database: Path = project_dir / "orders.duckdb"
    target: str = {
        True: f'[connection]\ndatabase = "{database}"\n',
        False: '[defaults]\ndatabase = "example"\n',
    }[adapter == "duckdb"]
    files: dict[str, str] = {
        "sqlbuild_project.toml": (
            f'name = "orders"\nadapter = "{adapter}"\n{target}[rules]\nselect = ["SQLCLEAN020"]\n'
        ),
    }
    for model in models:
        files[f"models/{model.name}.sql"] = (
            "MODEL (description 'Test model.', materialized view, schema analytics);\n\n"
            f"{model.query_sql}\n"
        )
    prepare_inline_project(
        tmp_path=project_dir.parent, project_name=project_dir.name, repo_files=files
    )
    return database


def compile_set_operation_project(*, project_dir: Path) -> SetOperationCompileResult:
    """Compile without cache and return diagnostics and per-model output column counts."""

    result: subprocess.CompletedProcess[str] = run_sqb(
        project_dir=project_dir, command=("compile", "--no-cache", "--json")
    )
    payload: dict[str, Any] = json.loads(result.stdout)
    return SetOperationCompileResult(
        exit_code=result.returncode,
        diagnostics=tuple((item["code"], item["message"]) for item in payload["diagnostics"]),
        column_counts={
            model["name"]: model["column_count"] for model in payload["resources"]["models"]
        },
        output=result.stdout + result.stderr,
    )


def _indented_set_operation_lines(path: Path) -> list[str]:
    lines: list[str] = _INDENTED_SET_OPERATION_PATTERN.findall(path.read_text(encoding="utf-8"))
    return [f"{path.name}: {line}" for line in lines]


def run_set_operation_lifecycle(
    *, project_dir: Path, models: tuple[SetOperationModel, ...]
) -> SetOperationLifecycleResult:
    """Compile, build, run Rules, format, and recompile a set-operation project."""

    database: Path = write_set_operation_project(project_dir=project_dir, models=models)
    compiled: SetOperationCompileResult = compile_set_operation_project(project_dir=project_dir)
    build: subprocess.CompletedProcess[str] = run_sqb(project_dir=project_dir, command=("build",))
    relation_shapes: dict[str, tuple[int, int]] = {}
    with duckdb.connect(str(database), read_only=True) as connection:
        for model in models:
            relation: duckdb.DuckDBPyRelation = connection.sql(
                f"SELECT * FROM analytics.{model.name}"
            )
            relation_shapes[model.name] = (len(relation.columns), len(relation.fetchall()))
    rules: subprocess.CompletedProcess[str] = run_set_arity_rule(project_dir=project_dir)
    formatted: subprocess.CompletedProcess[str]
    indented_set_operation_lines: tuple[str, ...]
    formatted, indented_set_operation_lines = format_set_operation_models(project_dir=project_dir)
    return SetOperationLifecycleResult(
        compiled=compiled,
        build=build,
        relation_shapes=relation_shapes,
        rules=rules,
        format=formatted,
        indented_set_operation_lines=indented_set_operation_lines,
        recompiled=compile_set_operation_project(project_dir=project_dir),
    )


def run_set_arity_rule(*, project_dir: Path) -> subprocess.CompletedProcess[str]:
    """Run the set-operation arity Rule over every model."""

    return run_sqb(project_dir=project_dir, command=("rules", "run", "SQLCLEAN020"))


def format_set_operation_models(
    *, project_dir: Path
) -> tuple[subprocess.CompletedProcess[str], tuple[str, ...]]:
    """Format every model and return the result and any set-operation lines left indented."""

    models_dir: Path = project_dir / "models"
    formatted: subprocess.CompletedProcess[str] = run_sqb(
        project_dir=project_dir, command=("format", str(models_dir))
    )
    indented_set_operation_lines: list[str] = []
    for path in sorted(models_dir.glob("*.sql")):
        indented_set_operation_lines.extend(_indented_set_operation_lines(path))
    return formatted, tuple(indented_set_operation_lines)


def column_lineage_edges(
    *, project_dir: Path, target: str
) -> tuple[subprocess.CompletedProcess[str], tuple[tuple[str, str, str], ...]]:
    """Return the lineage result and sorted `(model, column, transform)` upstream edges."""

    result: subprocess.CompletedProcess[str] = run_sqb(
        project_dir=project_dir, command=("lineage", target, "--json")
    )
    payload: dict[str, Any] = json.loads(result.stdout or '{"trace": []}')
    edges: list[tuple[str, str, str]] = [
        (edge["source"]["resource_name"], edge["source"]["column_name"], edge["transform"])
        for edge in payload["trace"]
    ]
    return result, tuple(sorted(edges))


class UnionFixtureCompileMeasurement(NamedTuple):
    analysis_native_ms: int
    sql_tests: int
    errors: int


def _union_all_fixture_rows(*, row_count: int, status: str, separator: str) -> str:
    return separator.join(
        f"  SELECT {row} AS order_id, CAST({row} AS DOUBLE) AS amount, '{status}' AS status"
        for row in range(row_count)
    )


def write_union_fixture_test_project(
    *,
    project_dir: Path,
    sql_test_count: int,
    fixture_row_count: int,
    input_fixture_row_separator: str,
) -> None:
    """Write a DuckDB project whose SQL tests use long UNION ALL fixture chains.

    `input_fixture_row_separator` joins the rows of the mocked input fixture; the
    expected fixture always uses plain `UNION ALL`.
    """

    files: dict[str, str] = {
        "sqlbuild_project.toml": (
            f'name = "orders"\nadapter = "duckdb"\n[connection]\n'
            f'database = "{project_dir / "orders.duckdb"}"\n[rules]\nselect = []\n'
        ),
        "models/base_orders.sql": (
            "MODEL (description 'Test model base_orders.', materialized view);\n\n"
            "SELECT 1 AS order_id, CAST(1 AS DOUBLE) AS amount, 'open' AS status\n"
        ),
        "models/orders.sql": (
            "MODEL (description 'Test model orders.', materialized view);\n\n"
            'SELECT b.order_id, b.amount, b.status FROM __ref("base_orders") AS b\n'
        ),
    }
    for index in range(sql_test_count):
        status: str = f"status_{index:02d}"
        input_rows: str = _union_all_fixture_rows(
            row_count=fixture_row_count, status=status, separator=input_fixture_row_separator
        )
        expected_rows: str = _union_all_fixture_rows(
            row_count=fixture_row_count, status=status, separator=" UNION ALL\n"
        )
        files[f"tests/unit/orders_case_{index:02d}.sql"] = (
            f'TEST (name "orders_case_{index:02d}");\n\n'
            f"WITH\n__ref__base_orders AS (\n{input_rows}\n),\n"
            f"__expected__orders AS (\n{expected_rows}\n)\nSELECT 1\n"
        )
    prepare_inline_project(
        tmp_path=project_dir.parent, project_name=project_dir.name, repo_files=files
    )


def measure_union_fixture_compile(
    *, project_dir: Path, runs: int
) -> UnionFixtureCompileMeasurement:
    """Return the fastest native analysis time of fresh uncached compiles."""

    skip_actions: dict[bool, Callable[[], None]] = {
        False: _continue_compile_benchmark,
        True: _skip_compile_benchmark,
    }
    skip_actions[os.environ.get("SQLBUILD_SKIP_PERFORMANCE_TESTS") == "1"]()
    measurements: list[UnionFixtureCompileMeasurement] = []
    for _ in range(runs):
        result: subprocess.CompletedProcess[str] = run_installed_sqb(
            project_dir=project_dir, args=("compile", "--no-cache", "--json"), env={}
        )
        assert result.returncode == 0, result.stdout + result.stderr
        payload: dict[str, Any] = json.loads(result.stdout)
        measurements.append(
            UnionFixtureCompileMeasurement(
                analysis_native_ms=payload["compile_timings"]["analysis_native_ms"],
                sql_tests=payload["summary"]["tests"],
                errors=payload["summary"]["errors"],
            )
        )
    return min(measurements)


REQUIRED_DESCRIPTIONS_PROJECT: str = (
    'name = "orders"\nadapter = "duckdb"\n\n[connection]\ndatabase = "orders.duckdb"\n'
)
_REQUIRED_DESCRIPTIONS_ORDERS_MODEL: str = (
    'MODEL (description "One row per order");\n\nSELECT 1 AS order_id\n'
)
_REQUIRED_DESCRIPTIONS_COMPILE: tuple[str, ...] = ("--no-color", "compile", "--json", "--no-cache")


def hooked_model_file(*, model_name: str, hook: str) -> tuple[str, str]:
    """Return a described model file whose post hook references ``hook``."""

    return (
        f"models/{model_name}.sql",
        f'MODEL (description "Orders with a hook", post_hooks [{hook}]);\n\nSELECT 1 AS order_id\n',
    )


def python_node_source(
    *,
    module: str,
    decorator: str,
    name: str,
    docstring_line: str = "",
    extra_import: str = "",
    decorator_arguments: str = "",
) -> str:
    """Render one decorated Python node module for required-description E2Es."""

    return (
        f"from sqlbuild.{module} import {decorator}\n{extra_import}\n\n"
        f"@{decorator}{decorator_arguments}\ndef {name}(ctx):\n{docstring_line}    return None\n"
    )


def compile_inline_files(
    *, tmp_path: Path, files: tuple[tuple[str, str], ...]
) -> subprocess.CompletedProcess[str]:
    """Compile a described orders project extended with ``files``."""

    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="orders",
        repo_files={
            "sqlbuild_project.toml": REQUIRED_DESCRIPTIONS_PROJECT,
            "models/orders.sql": _REQUIRED_DESCRIPTIONS_ORDERS_MODEL,
            **dict(files),
        },
    )
    return run_sqb(project_dir=project_dir, command=_REQUIRED_DESCRIPTIONS_COMPILE)


def compile_json_payload(*, project_dir: Path) -> tuple[int, dict[str, Any]]:
    """Run one JSON compile through the CLI and return its exit code and payload."""

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "compile", "--json"), project_dir=project_dir
    )
    return result.returncode, json.loads(result.stdout)


def compile_json_with(
    *, project_dir: Path, arguments: tuple[str, ...]
) -> tuple[int, dict[str, Any]]:
    """Run one JSON compile with extra arguments through the CLI and return its exit and payload."""

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "compile", "--json", *arguments), project_dir=project_dir
    )
    return result.returncode, json.loads(result.stdout)


def early_rules_outcome(payload: dict[str, Any]) -> tuple[int, int]:
    """The early built-in rules hits and misses one compile payload reports."""

    timings: dict[str, Any] = payload["compile_timings"]
    return timings["early_rules_hits"], timings["early_rules_misses"]


def rule_cache_outcome(payload: dict[str, Any]) -> tuple[int, int]:
    """The rule cache hits and misses one compile payload reports."""

    timings: dict[str, Any] = payload["compile_timings"]
    return timings["rule_cache_hits"], timings["rule_cache_misses"]


def diagnostic_location_keys(payload: dict[str, Any]) -> tuple[tuple[str, str, int, int], ...]:
    """Sorted code, path, line, and column of every diagnostic in a compile payload."""

    return tuple(
        sorted(
            (diagnostic["code"], diagnostic["path"], diagnostic["line"], diagnostic["column"])
            for diagnostic in payload["diagnostics"]
        )
    )


COMPILE_REUSE_REGION_ENV_VAR: str = "ORDERS_REGION"
COMPILE_REUSE_ENV: dict[str, str] = {
    REUSE_DISABLE_ENV_VAR: "0",
    COMPILE_CACHE_REGION_ENV_VAR: "east",
    COMPILE_REUSE_REGION_ENV_VAR: "north",
}
COMPILE_REUSE_HIT_LINE: str = "Inputs unchanged; reused the previous compile"
_COMPILE_REUSE_RULES_CONFIG: str = (
    '\n[rules]\nselect = ["XSQBR"]\n\n[rules.thresholds]\nmin_custom_rule_test_cases = 0\n'
    "\n[references]\nenforce_explicit = false\n"
)
_FUTURE_MTIME_OFFSET_NS: int = 5_000_000_000
_PHASE_DURATION: re.Pattern[str] = re.compile(r"  \(\d+\.\d+s\)")
_PROGRESS_LINE: re.Pattern[str] = re.compile(r".+  (START|OK  \(\d+\.\d+s\))")
_COMPILE_TIMINGS_PATTERN: re.Pattern[str] = re.compile(r'\n  "compile_timings": \{[^{}]*\n  \}')
_COMPILE_REUSE_EXTRA_PROJECT_FILES: dict[str, str] = {
    "macros/python/_rounding.py": (
        "_SCALE: int = 2\n\n\ndef scale() -> int:\n"
        '    """Return the rounding scale."""\n    return _SCALE\n'
    ),
    "macros/python/currency.py": (
        '"""Project-wide currency macros."""\n\nfrom macros.python._rounding import scale\n\n\n'
        "def line_total_cents(price_cents: str, quantity: str) -> str:\n"
        '    """Calculate line total cents from a unit price and quantity."""\n'
        '    return f"ROUND({price_cents} * {quantity}, {scale()})"\n'
    ),
    "models/marts/regional_orders.sql": (
        "MODEL (description 'Orders tagged with the configured region.');\n\n"
        f"SELECT order_id, '@@ENV:{COMPILE_REUSE_REGION_ENV_VAR}' AS region "
        'FROM __ref("stg_orders")\n'
    ),
    "models/marts/_sqlbuild/_macros/python/order_source.py": (
        '"""Mart relation macros."""\n\n\n'
        "def order_source() -> str:\n"
        '    """Return the relation orders are read from."""\n'
        "    return '__ref(\"stg_orders\")'\n"
    ),
    "models/marts/referenced_orders.sql": (
        "MODEL (description 'Orders read through a direct reference.');\n\n"
        'SELECT order_id FROM __ref("stg_orders")\n'
    ),
    "models/marts/macro_sourced_orders.sql": (
        "MODEL (description 'Orders read through a macro-built reference.');\n\n"
        "SELECT order_id FROM @py.order_source()\n"
    ),
    "models/marts/var_sourced_orders.sql": (
        "MODEL (description 'Orders read through a variable reference.');\n\n"
        'SELECT order_id FROM __ref("@@orders_model")\n'
    ),
    "rules/limits.py": "MAX_NAME_LENGTH: int = 40\n",
    "rules/naming.py": (
        "from sqlbuild.rules import Finding, Model, RuleContext, rule\n\n"
        "from rules import limits\n\n\n"
        '@rule(code="XSQBRNAME001", message="Model names stay short", '
        'remediation="Rename the model.")\n'
        "def short_names(*, model: Model, ctx: RuleContext) -> list[Finding]:\n"
        "    if len(model.name) <= limits.MAX_NAME_LENGTH:\n"
        "        return []\n"
        "    return [ctx.finding(subject=model)]\n"
    ),
}


class CompileReuseRun(NamedTuple):
    """Comparable result of one fresh-process JSON compile with compile reuse enabled."""

    returncode: int
    report: str
    stderr: str
    timings: dict[str, int]
    compiled: dict[str, bytes]

    @property
    def reused(self) -> bool:
        """Return whether the run replayed the stored compile."""

        return COMPILE_REUSE_HIT_LINE in self.stderr


def prepare_compile_reuse_project(*, project_dir: Path) -> None:
    """Create a project whose inputs cover every kind compile reuse fingerprints."""

    prepare_compile_cache_invalidation_project(project_dir=project_dir)
    for relative_path, contents in _COMPILE_REUSE_EXTRA_PROJECT_FILES.items():
        write_project_file(project_dir, relative_path, contents)
    replace_project_text(
        project_dir,
        "sqlbuild_project.toml",
        'quantity_multiplier = "2"',
        'quantity_multiplier = "2"\norders_model = "stg_orders"',
    )
    config_path: Path = project_dir / "sqlbuild_project.toml"
    config_path.write_text(
        config_path.read_text(encoding="utf-8") + _COMPILE_REUSE_RULES_CONFIG, encoding="utf-8"
    )
    (project_dir / "waffle_shop_control.duckdb").write_bytes(b"original database pages")


def run_reuse_compile(
    *,
    project_dir: Path,
    env: dict[str, str] | None = None,
    args: tuple[str, ...] = (),
    global_args: tuple[str, ...] = (),
) -> CompileReuseRun:
    """Compile with --json in a fresh process with reuse enabled and capture comparable output."""

    result: subprocess.CompletedProcess[str] = run_installed_sqb(
        project_dir=project_dir,
        args=(*global_args, "compile", "--json", *args),
        env={**COMPILE_REUSE_ENV, **(env or {})},
    )
    payload: dict[str, object] = cast(dict[str, object], json.loads(result.stdout))
    return CompileReuseRun(
        returncode=result.returncode,
        report=_COMPILE_TIMINGS_PATTERN.sub("", result.stdout),
        stderr=result.stderr,
        timings=cast(dict[str, int], payload.get("compile_timings", {})),
        compiled=compiled_artifacts(project_dir=project_dir),
    )


def run_reuse_text_compile(
    *, project_dir: Path, args: tuple[str, ...]
) -> subprocess.CompletedProcess[str]:
    """Compile with the text report in a fresh process with reuse enabled."""

    return run_installed_sqb(
        project_dir=project_dir, args=("compile", *args), env=COMPILE_REUSE_ENV
    )


def run_reuse_compile_into_file(
    *, project_dir: Path, report_path: Path
) -> subprocess.CompletedProcess[str]:
    """Compile with --json while the shell-style stdout redirect targets a project file."""

    with report_path.open("w", encoding="utf-8") as report:
        return subprocess.run(
            [
                str(Path(sys.executable).with_name("sqb")),
                "--project-dir",
                str(project_dir),
                "--no-color",
                "compile",
                "--json",
            ],
            stdout=report,
            stderr=subprocess.PIPE,
            text=True,
            env={**os.environ, **COMPILE_REUSE_ENV},
            check=False,
        )


def compile_reuse_hit_count(*, report_path: Path) -> int:
    """Return the reuse hit counter of a JSON report written to a file."""

    payload: dict[str, object] = cast(
        dict[str, object], json.loads(report_path.read_text(encoding="utf-8"))
    )
    return cast(dict[str, int], payload["compile_timings"])["project_reuse_hits"]


def stderr_without_durations(*, stderr: str) -> str:
    """Return compile stderr without its phase durations."""

    return _PHASE_DURATION.sub("", stderr)


def compile_notes(*, stderr: str) -> list[str]:
    """Return the note lines a compile printed to stderr."""

    return re.findall(r"^note:.*$", stderr, flags=re.MULTILINE)


def progress_only(*, text: str) -> bool:
    """Return whether text consists only of per-phase progress lines."""

    return all(_PROGRESS_LINE.fullmatch(line) is not None for line in text.splitlines())


def write_recording_sink(*, project_dir: Path, sink_name: str) -> None:
    """Add a lifecycle sink that appends invocation events to ORDERS_EVENT_PATH."""

    write_project_file(
        project_dir,
        f"sinks/{sink_name}.py",
        "import os\nfrom pathlib import Path\n\n"
        "from sqlbuild.sinks import (\n    LifecycleEvent,\n    LifecycleEventKind,\n"
        "    lifecycle_event_sink,\n    lifecycle_event_to_json,\n)\n\n\n"
        f'@lifecycle_event_sink(name="{sink_name}", '
        "event_kinds={LifecycleEventKind.INVOCATION})\n"
        "def record_event(event: LifecycleEvent) -> None:\n"
        '    with Path(os.environ["ORDERS_EVENT_PATH"]).open("a", encoding="utf-8") as stream:\n'
        '        stream.write(lifecycle_event_to_json(event) + "\\n")\n',
    )


def recorded_events(*, path: Path) -> list[dict[str, object]]:
    """Read the lifecycle events a recording sink appended."""

    return [
        cast(dict[str, object], json.loads(line))
        for line in path.read_text(encoding="utf-8").splitlines()
    ]


def compile_reuse_entry_paths(*, project_dir: Path) -> tuple[Path, ...]:
    """Return every stored compile reuse slot of a project."""

    return tuple(
        sorted(
            (compiler_cache_directory(project_dir) / NATIVE_REUSE_DIRECTORY_NAME).glob(
                f"*{NATIVE_REUSE_SUFFIX}"
            )
        )
    )


def touch_model_without_change(project_dir: Path) -> None:
    """Move a model's mtime forward without changing its content."""

    path: Path = project_dir / "models/staging/stg_orders.sql"
    mtime_ns: int = path.stat().st_mtime_ns + _FUTURE_MTIME_OFFSET_NS
    os.utime(path, ns=(mtime_ns, mtime_ns))


def rewrite_macro_without_change(project_dir: Path) -> None:
    """Rewrite a macro file with identical bytes."""

    path: Path = project_dir / "macros/python/currency.py"
    path.write_bytes(path.read_bytes())


def delete_compiled_model(project_dir: Path) -> None:
    """Delete one compiled artifact by hand."""

    (project_dir / "target/compiled/models/marts/regional_orders.sql").unlink()


def edit_compiled_model(project_dir: Path) -> None:
    """Edit one compiled artifact by hand."""

    path: Path = project_dir / "target/compiled/models/marts/regional_orders.sql"
    path.write_text(path.read_text(encoding="utf-8") + "-- edited by hand\n", encoding="utf-8")


def add_stale_compiled_file(project_dir: Path) -> None:
    """Add a compiled artifact that no model produces."""

    write_project_file(project_dir, "target/compiled/models/stale_model.sql", "SELECT 1\n")


COMPILED_TREE_STAMP_FILE_NAME: str = "compiled-tree.bin"


def compiled_tree_stamp_path(*, project_dir: Path) -> Path:
    """Return the compile cache file stamping the compiled tree the last stale-file walk left."""

    return compiler_cache_directory(project_dir) / COMPILED_TREE_STAMP_FILE_NAME


def add_stale_compiled_file_beside_models(project_dir: Path) -> None:
    """Add an unmanaged compiled file to a folder that already holds compiled models."""

    write_project_file(project_dir, "target/compiled/models/marts/stale_mart.sql", "SELECT 1\n")


def link_compiled_root(project_dir: Path) -> None:
    """Make `target/compiled` a link to another folder under `target/`."""

    linked: Path = project_dir / "target/compiled-linked"
    linked.mkdir(parents=True, exist_ok=True)
    (project_dir / "target/compiled").symlink_to(linked, target_is_directory=True)


def add_stale_file_in_compiled_root(project_dir: Path) -> None:
    """Add an unmanaged file directly in the compiled folder."""

    write_project_file(project_dir, "target/compiled/stray.sql", "SELECT 1\n")


def truncate_file(path: Path) -> None:
    """Drop the last bytes of a stored entry."""

    path.write_bytes(path.read_bytes()[:-7])


def flip_trailing_bytes(path: Path) -> None:
    """Corrupt the stored stdout section without changing its length."""

    path.write_bytes(path.read_bytes()[:-3] + b"xyz")


def flip_header_byte(path: Path) -> None:
    """Corrupt one byte inside the stored inputs section."""

    contents: bytes = path.read_bytes()
    path.write_bytes(contents[:40] + b"!" + contents[41:])


def stored_output_path(entry: Path) -> Path:
    """Return the output file stored beside one reuse slot file."""

    return entry.with_suffix(NATIVE_REUSE_OUTPUT_SUFFIX)


def truncate_stored_output(entry: Path) -> None:
    """Drop the last bytes of the slot's stored output."""

    truncate_file(stored_output_path(entry))


def flip_stored_stdout_bytes(entry: Path) -> None:
    """Corrupt the stored report inside the slot's output without changing its length."""

    flip_trailing_bytes(stored_output_path(entry))


def remove_stored_output(entry: Path) -> None:
    """Delete the slot's stored output, leaving its inputs."""

    stored_output_path(entry).unlink()


def garbage_stored_output(entry: Path) -> None:
    """Replace the slot's stored output with unrelated bytes."""

    garbage_file(stored_output_path(entry))


def previous_format_slot(entry: Path) -> None:
    """Leave a slot as the previous single-file store format wrote it."""

    entry.write_bytes(b"SQBSTORE" + (1).to_bytes(4, "little") + bytes(32) + b"project-reuse")


def empty_file(path: Path) -> None:
    """Leave an empty stored entry, as an interrupted copy would."""

    path.write_bytes(b"")


def garbage_file(path: Path) -> None:
    """Replace a stored entry with unrelated bytes."""

    path.write_bytes(b"not a stored compile")


ORDERS_API_TOKEN_ENV_VAR: str = "ORDERS_API_TOKEN"
ORDERS_API_TIMEOUT_ENV_VAR: str = "ORDERS_API_TIMEOUT_SECONDS"
_ORDERS_API_ENV_FILE_NAME: str = "orders_api.env"


def orders_api_env_file(project_dir: Path) -> Path:
    """Return the provider env file kept beside, not inside, the project."""

    return project_dir.parent / _ORDERS_API_ENV_FILE_NAME


def write_orders_api_provider(*, project_dir: Path) -> None:
    """Add a provider whose required settings come from the environment and an env file."""

    write_project_file(
        project_dir,
        "providers/orders_api.py",
        "from pydantic_settings import SettingsConfigDict\n"
        "from sqlbuild.providers import Provider\n\n\n"
        "class OrdersApi(Provider):\n"
        '    """Orders API connection settings."""\n\n'
        "    model_config = SettingsConfigDict(\n"
        f"        env_file={str(orders_api_env_file(project_dir))!r}\n"
        "    )\n\n"
        "    orders_api_token: str\n"
        "    orders_api_timeout_seconds: int = 30\n",
    )


def write_custom_source_provider(project_dir: Path) -> None:
    """Add a provider whose settings sources reuse cannot enumerate."""

    write_project_file(
        project_dir,
        "providers/orders_vault.py",
        "from pydantic_settings import BaseSettings, PydanticBaseSettingsSource\n"
        "from sqlbuild.providers import Provider\n\n\n"
        "class OrdersVault(Provider):\n"
        '    """Orders vault settings read from a custom source."""\n\n'
        '    vault_path: str = "orders"\n\n'
        "    @classmethod\n"
        "    def settings_customise_sources(\n"
        "        cls,\n"
        "        settings_cls: type[BaseSettings],\n"
        "        init_settings: PydanticBaseSettingsSource,\n"
        "        env_settings: PydanticBaseSettingsSource,\n"
        "        dotenv_settings: PydanticBaseSettingsSource,\n"
        "        file_secret_settings: PydanticBaseSettingsSource,\n"
        "    ) -> tuple[PydanticBaseSettingsSource, ...]:\n"
        "        return (init_settings,)\n",
    )


def write_orders_api_timeout_file(project_dir: Path) -> None:
    """Change a provider setting through its env file only."""

    orders_api_env_file(project_dir).write_text(
        f"{ORDERS_API_TIMEOUT_ENV_VAR}=45\n", encoding="utf-8"
    )


def enable_compile_reuse(monkeypatch: pytest.MonkeyPatch) -> None:
    """Enable compile reuse and the fixture environment for in-process compiles."""

    for name, value in COMPILE_REUSE_ENV.items():
        monkeypatch.setenv(name, value)


def compile_in_process(*, project_dir: Path) -> int:
    """Run one JSON compile through the CLI entry point in this process."""

    with redirect_stdout(StringIO()):
        return main(["--project-dir", str(project_dir), "--no-color", "compile", "--json"])


def compile_edit_without_reuse(project_dir: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    """Compile an edit with compile reuse disabled, so only the analysis cache sees it."""

    monkeypatch.setenv(REUSE_DISABLE_ENV_VAR, "1")
    _ = compile_in_process(project_dir=project_dir)
    monkeypatch.setenv(REUSE_DISABLE_ENV_VAR, "0")


def write_before_reuse_store(
    *, monkeypatch: pytest.MonkeyPatch, project_dir: Path, write: Callable[[Path], None]
) -> None:
    """Let another writer change target/ once, after a compile wrote it and before it stores."""

    pending: list[Callable[[Path], None]] = [write]

    class WrittenThenChanged(native_module.PreparedCompileArtifacts):
        def __exit__(self, *exception: object) -> None:
            super().__exit__(*exception)
            for action in tuple(pending):
                action(project_dir)
            pending.clear()

    monkeypatch.setattr(native_module, "PreparedCompileArtifacts", WrittenThenChanged)


def edit_project_model_during_compile(project_dir: Path) -> None:
    """Change one model's bytes after the compile read it and before it stores."""

    path: Path = project_dir / "models/staging/stg_orders.sql"
    path.write_bytes(path.read_bytes() + b"\n")


def rewrite_compiled_model_unchanged(project_dir: Path) -> None:
    """Rewrite one compiled artifact with the bytes it already holds."""

    path: Path = project_dir / "target/compiled/models/marts/regional_orders.sql"
    path.write_bytes(path.read_bytes())


class _DigestedFilesHandler(logging.Handler):
    """Collect the project-file digest counts compile reuse logs at debug level."""

    def __init__(self, reads: list[int]) -> None:
        super().__init__(level=logging.DEBUG)
        self._reads: list[int] = reads

    def emit(self, record: logging.LogRecord) -> None:
        digest: bool = "digested %d project files" in str(record.msg)
        self._reads.extend(int(count) for count in cast(tuple[int], record.args)[: int(digest)])


def record_digested_files(*, monkeypatch: pytest.MonkeyPatch) -> list[int]:
    """Record how many project files each compile reuse check and record digested."""

    reads: list[int] = []
    logger: logging.Logger = logging.getLogger(REUSE_LOGGER_NAME)
    monkeypatch.setattr(logger, "handlers", [*logger.handlers, _DigestedFilesHandler(reads)])
    monkeypatch.setattr(logger, "level", logger.level)
    logger.setLevel(logging.DEBUG)
    return reads


def settle_racy_window(*, monkeypatch: pytest.MonkeyPatch) -> None:
    """Treat files written before a compile starts as settled instead of waiting two seconds."""

    monkeypatch.setattr(native_module, "COMPILE_REUSE_RACY_WINDOW_NS", 0)


def write_large_file(*, project_dir: Path, relative_path: str, size_bytes: int) -> Path:
    """Write a large data file that no compile reads."""

    path: Path = project_dir / relative_path
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(b"0" * size_bytes)
    return path


def touch_back(path: Path, *, seconds: int) -> None:
    """Set a file's mtime to a past time without changing its content, as a checkout can."""

    mtime_ns: int = time.time_ns() - seconds * 1_000_000_000
    os.utime(path, ns=(mtime_ns, mtime_ns))


def compiled_artifacts(*, project_dir: Path) -> dict[str, bytes]:
    """Return every compiled and failed-compile SQL artifact by path relative to target/."""

    target_dir: Path = project_dir / "target"
    paths: list[Path] = [
        *sorted((target_dir / "compiled").rglob("*.sql")),
        *sorted((target_dir / "compiled-failed").rglob("*.sql")),
    ]
    return {path.relative_to(target_dir).as_posix(): path.read_bytes() for path in paths}


def compile_in_process_reused(
    *, project_dir: Path, capsys: pytest.CaptureFixture[str]
) -> tuple[int, bool]:
    """Compile in this process and return its exit code and whether it reused."""

    _ = capsys.readouterr()
    code: int = compile_in_process(project_dir=project_dir)
    return code, COMPILE_REUSE_HIT_LINE in capsys.readouterr().err


def in_process_compile_reads(
    *, project_dir: Path, digested: list[int], capsys: pytest.CaptureFixture[str]
) -> tuple[int, int, bool]:
    """Compile in this process; return its exit code, project files read, and whether it reused."""

    start: int = len(digested)
    _ = capsys.readouterr()
    code: int = compile_in_process(project_dir=project_dir)
    reused: bool = COMPILE_REUSE_HIT_LINE in capsys.readouterr().err
    reads: int = sum(digested[start:])
    return code, reads, reused


def fail_reuse_store(*, monkeypatch: pytest.MonkeyPatch, error: BaseException) -> None:
    """Make storing a finished compile raise while enumerating provider settings."""

    def raise_error(**_kwargs: Any) -> None:
        raise error

    monkeypatch.setattr(provider_settings, "provider_settings_inputs", raise_error)


def compile_in_process_output(
    *, project_dir: Path, capsys: pytest.CaptureFixture[str], args: tuple[str, ...] = ()
) -> tuple[int, str, str]:
    """Run one JSON compile in this process and return its exit code, stdout, and stderr."""

    _ = capsys.readouterr()
    code: int = main(["--project-dir", str(project_dir), "--no-color", "compile", "--json", *args])
    out, err = capsys.readouterr()
    return code, out, err


class IncrementalEditComparison(NamedTuple):
    """An incremental compile and the --no-cache compile of the same edited project."""

    incremental: CompileReuseRun
    reference: CompileReuseRun

    @property
    def matches(self) -> bool:
        """Return whether exit code, report, and every compiled artifact are byte-identical."""

        return _edit_outcome(self.incremental) == _edit_outcome(self.reference)

    @property
    def mismatched_artifacts(self) -> list[str]:
        """Return the first compiled artifacts that differ from the --no-cache compile."""

        incremental: dict[str, bytes] = self.incremental.compiled
        reference: dict[str, bytes] = self.reference.compiled
        return sorted(
            filter(
                lambda path: incremental.get(path) != reference.get(path),
                incremental.keys() | reference.keys(),
            )
        )[:5]


def _edit_outcome(run: CompileReuseRun) -> tuple[int, str, dict[str, bytes]]:
    return run.returncode, run.report, run.compiled


def compare_incremental_compile(*, project_dir: Path) -> IncrementalEditComparison:
    """Compile incrementally, then compile the same inputs without any cache."""

    incremental: CompileReuseRun = run_reuse_compile(project_dir=project_dir)
    reference: CompileReuseRun = run_reuse_compile(project_dir=project_dir, args=("--no-cache",))
    return IncrementalEditComparison(incremental=incremental, reference=reference)


def in_process_reuse_run(
    *, project_dir: Path, capsys: pytest.CaptureFixture[str], args: tuple[str, ...] = ()
) -> CompileReuseRun:
    """Compile with --json in this process and capture output comparable with fresh processes."""

    code, out, err = compile_in_process_output(project_dir=project_dir, capsys=capsys, args=args)
    payload: dict[str, object] = cast(dict[str, object], json.loads(out))
    return CompileReuseRun(
        returncode=code,
        report=_COMPILE_TIMINGS_PATTERN.sub("", out),
        stderr=err,
        timings=cast(dict[str, int], payload.get("compile_timings", {})),
        compiled=compiled_artifacts(project_dir=project_dir),
    )


GENERATED_EDIT_MODEL_PREFIX: str = "orders_step_"
_STAR_MODEL_SHARE: float = 0.4
_TWIN_MODEL_SHARE: float = 0.25
_GENERATED_EDIT_COLUMNS: tuple[str, ...] = ("order_id", "customer_id", "quantity", "status")
_PLAIN_QUANTITY: str = "  quantity,\n"
_CAST_QUANTITY: str = "  CAST(quantity AS BIGINT) AS quantity,\n"
_SWAP_PLACEHOLDER: str = "\x00swap\x00"
_RANDOM_EDIT_KINDS: tuple[str, ...] = (
    "comment",
    "comment",
    "add_column",
    "type_change",
    "header_change",
    "introduce_error",
    "drop_column",
    "add_model",
    "macro_edit",
    "test_edit",
)
_RANDOM_EDIT_FOLLOW_UPS: dict[str, tuple[str, ...]] = {
    "introduce_error": ("introduce_error", "fix_error"),
    "drop_column": ("drop_column", "comment", "restore_column"),
}
_DROPPED_COLUMN: str = "  customer_id,\n"
_KEPT_COLUMN: str = "  order_id,\n"
_EDIT_ANCHORS: dict[str, str] = {"type_change": "  quantity", "drop_column": _DROPPED_COLUMN}


def write_generated_edit_models(*, project_dir: Path, model_count: int, seed: int) -> None:
    """Add seeded models over the staging orders reading earlier ones, some with * or twins."""

    chooser: random.Random = random.Random(seed)
    star_chooser: random.Random = random.Random(seed + 2)
    upstreams: tuple[str, ...] = (
        "stg_orders",
        *(
            f"{GENERATED_EDIT_MODEL_PREFIX}{chooser.randrange(index):03d}"
            for index in range(1, model_count)
        ),
    )
    twin_chooser: random.Random = random.Random(seed + 3)
    for index, upstream in enumerate(upstreams):
        name: str = f"models/generated/{GENERATED_EDIT_MODEL_PREFIX}{index:03d}"
        contents: str = (_generated_edit_model_sql, _generated_star_model_sql)[
            star_chooser.random() < _STAR_MODEL_SHARE
        ](upstream=upstream)
        write_project_file(project_dir, f"{name}.sql", contents)
        twins: tuple[str, ...] = (contents,) * (twin_chooser.random() < _TWIN_MODEL_SHARE)
        for twin in twins:
            write_project_file(
                project_dir,
                f"{name}_twin.sql",
                twin.replace("MODEL (description '", "MODEL (description 'Twin. ", 1),
            )


def _generated_star_model_sql(*, upstream: str) -> str:
    return (
        "MODEL (description 'Generated order pass-through.', materialized view);\n\n"
        f'SELECT\n  *\nFROM __ref("{upstream}")\n'
    )


def _generated_edit_model_sql(*, upstream: str) -> str:
    columns: str = ",\n".join(f"  {column}" for column in _GENERATED_EDIT_COLUMNS)
    return (
        "MODEL (description 'Generated order step.', materialized view);\n\n"
        f'SELECT\n{columns}\nFROM __ref("{upstream}")\n'
    )


def random_edit_plan(*, seed: int, step_count: int) -> tuple[str, ...]:
    """Draw seeded edit kinds; every introduced error is fixed by the following step."""

    chooser: random.Random = random.Random(seed)
    return tuple(
        itertools.chain.from_iterable(
            map(
                lambda drawn: _RANDOM_EDIT_FOLLOW_UPS.get(drawn, (drawn,)),
                chooser.choices(_RANDOM_EDIT_KINDS, k=step_count),
            )
        )
    )


def _swapped(contents: str, first: str, second: str) -> str:
    return (
        contents.replace(first, _SWAP_PLACEHOLDER)
        .replace(second, first)
        .replace(_SWAP_PLACEHOLDER, second)
    )


class RandomEditChain:
    """Apply seeded random edits of every supported kind to a generated project."""

    def __init__(self, *, project_dir: Path, seed: int) -> None:
        self._project_dir: Path = project_dir
        self._random: random.Random = random.Random(seed)
        self._broken: Path = project_dir / "models" / "generated" / "unbroken.sql"
        self._dropped: Path = project_dir / "models" / "generated" / "undropped.sql"
        self._added: int = 0
        self._interventions: random.Random = random.Random(seed + 1)

    def intervene(self) -> None:
        """Sometimes run another command over the edited project before it is compiled."""

        commands: tuple[Callable[[Path], None], ...] = (
            *INTERVENING_COMMANDS,
            *(no_intervening_command,) * 3,
        )
        self._interventions.choice(commands)(self._project_dir)

    def apply(self, kind: str) -> None:
        """Apply one edit of the given kind to a randomly chosen generated model."""

        anchor: str = _EDIT_ANCHORS.get(kind, "")
        models: list[Path] = list(
            filter(
                lambda model: anchor in model.read_text(encoding="utf-8"),
                sorted((self._project_dir / "models" / "generated").glob("*.sql")),
            )
        )
        edit: Callable[[Path], None] = getattr(self, f"_{kind}")
        edit(self._random.choice(models))

    def _rewrite(self, model: Path, transform: Callable[[str], str]) -> None:
        model.write_text(transform(model.read_text(encoding="utf-8")), encoding="utf-8")

    def _comment(self, model: Path) -> None:
        note: str = f"-- note {self._random.random()}\nSELECT\n"
        self._rewrite(model, lambda contents: contents.replace("SELECT\n", note, 1))

    def _add_column(self, model: Path) -> None:
        column: str = f",\n  status AS status_{self._random.randrange(1000)}\nFROM __ref"
        self._rewrite(model, lambda contents: contents.replace("\nFROM __ref", column))

    def _type_change(self, model: Path) -> None:
        self._rewrite(model, lambda contents: _swapped(contents, _PLAIN_QUANTITY, _CAST_QUANTITY))

    def _header_change(self, model: Path) -> None:
        self._rewrite(
            model,
            lambda contents: _swapped(contents, "materialized view", "materialized table"),
        )

    def _introduce_error(self, model: Path) -> None:
        self._rewrite(
            model, lambda contents: contents.replace("SELECT\n", "SELECT\n  unknown_column,\n", 1)
        )
        self._broken = model

    def _fix_error(self, _model: Path) -> None:
        self._rewrite(self._broken, lambda contents: contents.replace("  unknown_column,\n", ""))

    def _drop_column(self, model: Path) -> None:
        self._rewrite(model, lambda contents: contents.replace(_DROPPED_COLUMN, "", 1))
        self._dropped = model

    def _restore_column(self, _model: Path) -> None:
        self._rewrite(
            self._dropped,
            lambda contents: contents.replace(_KEPT_COLUMN, _KEPT_COLUMN + _DROPPED_COLUMN, 1),
        )

    def _add_model(self, model: Path) -> None:
        self._added += 1
        write_project_file(
            self._project_dir,
            f"models/generated/added_step_{self._added:03d}.sql",
            _generated_edit_model_sql(upstream=model.stem),
        )

    def _macro_edit(self, _model: Path) -> None:
        scale: str = f"_SCALE: int = {2 + self._random.randrange(3)}"
        self._rewrite(
            self._project_dir / "macros" / "python" / "_rounding.py",
            lambda contents: re.sub(r"_SCALE: int = \d+", scale, contents),
        )

    def _test_edit(self, _model: Path) -> None:
        revision: str = f"\n-- revision {self._random.random()}\n"
        self._rewrite(
            self._project_dir / "tests" / "unit" / "test_stg_orders.sql",
            lambda contents: contents + revision,
        )


STG_ORDERS_MODEL: str = "models/staging/stg_orders.sql"
STG_PAYMENTS_MODEL: str = "models/staging/stg_payments.sql"
FACT_ORDERS_MODEL: str = "models/marts/fact_orders.sql"
DIM_CUSTOMERS_ARTIFACT: str = "target/compiled/models/marts/dim_customers.sql"
_FACT_QUANTITY_TYPE: str = "    quantity (type INTEGER),\n"
_FACT_ORDER_ID_COLUMN: str = "    order_id (nullable false, audits [not_null]),\n"


def no_intervening_command(_root: Path) -> None:
    """Compile the edit directly after making it."""


def edit_step(
    description: str,
    edit: Callable[[Path], None],
    between: Callable[[Path], None] = no_intervening_command,
) -> IncrementalEditStep:
    """Return one edit, optionally followed by another command before the compared compile."""

    return IncrementalEditStep(description=description, edit=edit, between=between)


def star_chain_steps(*, between: Callable[[Path], None]) -> tuple[IncrementalEditStep, ...]:
    """Add a star chain, then add and remove an upstream column with a command before each."""

    return (
        edit_step("star_chain_added", star_chain_added),
        edit_step("upstream_column_added", staging_extra_flag, between),
        edit_step("edit_after_added_column", fact_comment),
        edit_step("upstream_column_removed", staging_extra_flag_removed, between),
        edit_step("edit_after_removed_column", payments_comment),
    )


def plan_between(root: Path) -> None:
    """Plan the edited project before the compared compile."""

    _ = run_installed_sqb(project_dir=root, args=("plan",), env=COMPILE_REUSE_ENV)


def build_between(root: Path) -> None:
    """Build the edited project before the compared compile."""

    _ = run_installed_sqb(project_dir=root, args=("build",), env=COMPILE_REUSE_ENV)


def other_target_build_between(root: Path) -> None:
    """Build the edited project for another target before the compared default-target compile."""

    _ = run_installed_sqb(
        project_dir=root, args=("build", "--target", "prod"), env=COMPILE_REUSE_ENV
    )


def compile_without_reuse_between(root: Path) -> None:
    """Compile the edited project with compile reuse disabled before the compared compile."""

    _ = run_installed_sqb(
        project_dir=root,
        args=("compile", "--json"),
        env={**COMPILE_REUSE_ENV, REUSE_DISABLE_ENV_VAR: "1"},
    )


INTERVENING_COMMANDS: tuple[Callable[[Path], None], ...] = (
    plan_between,
    build_between,
    other_target_build_between,
    compile_without_reuse_between,
)
_STAR_CHAIN_MODELS: dict[str, str] = {
    "models/marts/chain_mid.sql": (
        "MODEL (description 'Every staged order column.', materialized view);\n\n"
        'SELECT * FROM __ref("stg_orders")\n'
    ),
    "models/marts/chain_leaf.sql": (
        "MODEL (description 'Every chained order column.', materialized view);\n\n"
        'SELECT * FROM __ref("chain_mid")\n'
    ),
}
_STAGING_LAST_COLUMN: str = "  status\nFROM __source"


_TWIN_MODEL: str = "models/marts/chain_a_twin.sql"


def star_chain_with_twin_added(root: Path) -> None:
    """Add the star chain and a twin of its leaf with the same SQL, so both share a cache key."""

    star_chain_added(root)
    write_project_file(root, _TWIN_MODEL, _STAR_CHAIN_MODELS["models/marts/chain_leaf.sql"])


def twin_header_changed(root: Path) -> None:
    """Change the twin's header only, leaving its SQL and so its analysis cache key unchanged."""

    replace_project_text(
        root,
        _TWIN_MODEL,
        "'Every chained order column.'",
        "'Every chained order column, twice.'",
    )


def star_chain_added(root: Path) -> None:
    """Add two models that each select every column of the one before, below staging orders."""

    for relative_path, contents in _STAR_CHAIN_MODELS.items():
        write_project_file(root, relative_path, contents)


def staging_extra_flag(root: Path) -> None:
    """Add an output column to the staging orders, which every star model passes on."""

    replace_project_text(
        root, STG_ORDERS_MODEL, _STAGING_LAST_COLUMN, "  status,\n  1 AS extra_flag\nFROM __source"
    )


def staging_extra_flag_removed(root: Path) -> None:
    """Remove the added staging orders output column again."""

    replace_project_text(
        root, STG_ORDERS_MODEL, "  status,\n  1 AS extra_flag\nFROM __source", _STAGING_LAST_COLUMN
    )


def staging_comment(root: Path) -> None:
    """Add a comment to the staging orders model without changing its shape."""

    replace_project_text(
        root,
        STG_ORDERS_MODEL,
        'FROM __source("raw__orders")',
        '-- staged\nFROM __source("raw__orders")',
    )


def staging_type_change(root: Path) -> None:
    """Change one staging output column type, which propagates downstream."""

    replace_project_text(
        root, STG_ORDERS_MODEL, "  quantity,\n", "  CAST(quantity AS BIGINT) AS quantity,\n"
    )


def staging_new_column(root: Path) -> None:
    """Add an output column to the staging orders model."""

    replace_project_text(
        root, STG_ORDERS_MODEL, "  status\n", "  status,\n  status AS raw_status\n"
    )


def staging_header_change(root: Path) -> None:
    """Change the staging orders materialization in its header."""

    replace_project_text(root, STG_ORDERS_MODEL, "materialized view,", "materialized table,")


def staging_contract_change(root: Path) -> None:
    """Add a declared type to one staging column contract."""

    replace_project_text(
        root,
        STG_ORDERS_MODEL,
        "customer_id (nullable false, audits [not_null]),",
        "customer_id (type BIGINT, nullable false, audits [not_null]),",
    )


def fact_audit_added(root: Path) -> None:
    """Attach one more column audit to the fact orders model."""

    replace_project_text(
        root,
        FACT_ORDERS_MODEL,
        "    order_id (nullable false, audits [not_null]),",
        "    order_id (nullable false, audits [not_null, unique]),",
    )


def fact_error_introduced(root: Path) -> None:
    """Make the fact orders model read a column that does not exist."""

    replace_project_text(root, FACT_ORDERS_MODEL, "  o.quantity,\n", "  o.missing_quantity,\n")


def fact_error_fixed(root: Path) -> None:
    """Restore the column the fact orders model reads."""

    replace_project_text(root, FACT_ORDERS_MODEL, "  o.missing_quantity,\n", "  o.quantity,\n")


def fact_comment(root: Path) -> None:
    """Add a comment to the leaf fact orders model."""

    replace_project_text(root, FACT_ORDERS_MODEL, "FROM __ref", "-- leaf\nFROM __ref")


def payment_expression_type_change(root: Path) -> None:
    """Widen one column of the payments source expression, which changes its inferred shape."""

    replace_project_text(
        root,
        "sources/raw.yml",
        "SELECT 1 AS id, 1 AS order_id,",
        "SELECT 1 AS id, CAST(1 AS BIGINT) AS order_id,",
    )


def completed_order_udf_signature_change(root: Path) -> None:
    """Change the completed-order UDF's declared return type, which its callers' shapes read."""

    replace_project_text(
        root,
        "functions/sql/udf__is_completed_order.sql",
        "returns BOOLEAN,\n);\n\norder_status = 'completed'",
        "returns INTEGER,\n);\n\nCAST(order_status = 'completed' AS INTEGER)",
    )


def order_status_nullability_change(root: Path) -> None:
    """Declare one raw orders source column non-null in its schema entry."""

    replace_project_text(
        root,
        "sources/raw.yml",
        "      - name: status\n        type: VARCHAR\n",
        "      - name: status\n        type: VARCHAR\n        nullable: false\n",
    )


def staging_column_removed(root: Path) -> None:
    """Drop the quantity column from staging orders, which downstream models and tests read."""

    replace_project_text(root, STG_ORDERS_MODEL, "  quantity,\n", "")


def staging_column_restored(root: Path) -> None:
    """Restore the staging orders quantity column."""

    replace_project_text(
        root, STG_ORDERS_MODEL, "  waffle_type_id,\n", "  waffle_type_id,\n  quantity,\n"
    )


def staging_column_renamed(root: Path) -> None:
    """Rename the staging orders status column that audits, tests, and models read."""

    replace_project_text(root, STG_ORDERS_MODEL, "  status\nFROM", "  status AS order_state\nFROM")


def staging_rename_reverted(root: Path) -> None:
    """Restore the staging orders status column name."""

    replace_project_text(root, STG_ORDERS_MODEL, "  status AS order_state\nFROM", "  status\nFROM")


def staging_quantity_text(root: Path) -> None:
    """Turn the staging orders quantity into text, which violates a downstream declared type."""

    replace_project_text(
        root, STG_ORDERS_MODEL, "  quantity,\n", "  CAST(quantity AS VARCHAR) AS quantity,\n"
    )


def staging_quantity_restored(root: Path) -> None:
    """Restore the staging orders quantity type."""

    replace_project_text(
        root, STG_ORDERS_MODEL, "  CAST(quantity AS VARCHAR) AS quantity,\n", "  quantity,\n"
    )


def fact_quantity_type_declared(root: Path) -> None:
    """Declare the fact orders quantity type, which enforces it."""

    replace_project_text(
        root, FACT_ORDERS_MODEL, _FACT_ORDER_ID_COLUMN, _FACT_ORDER_ID_COLUMN + _FACT_QUANTITY_TYPE
    )


def fact_quantity_type_removed(root: Path) -> None:
    """Remove the declared fact orders quantity type."""

    replace_project_text(root, FACT_ORDERS_MODEL, _FACT_QUANTITY_TYPE, "")


def payments_comment(root: Path) -> None:
    """Add a comment to the staging payments model, unrelated to staging orders."""

    anchor: str = 'FROM __source("raw__payments")'
    replace_project_text(root, STG_PAYMENTS_MODEL, anchor, f"-- paid\n{anchor}")


def compiled_artifact_tampered(root: Path) -> None:
    """Overwrite an unchanged model's compiled artifact, then edit another model."""

    artifact: Path = root / DIM_CUSTOMERS_ARTIFACT
    artifact.write_bytes(artifact.read_bytes() + b"-- tampered\n")
    staging_comment(root)


def stg_orders_test_edit(root: Path) -> None:
    """Change one expected value in the staging orders SQL test."""

    replace_project_text(
        root, "tests/unit/test_stg_orders.sql", "100 AS customer_id", "101 AS customer_id"
    )


def stg_orders_test_edited_again(root: Path) -> None:
    """Change the staging orders SQL test's edited expected value once more."""

    replace_project_text(
        root, "tests/unit/test_stg_orders.sql", "101 AS customer_id", "102 AS customer_id"
    )


def adapter_switched(*, before: str, after: str) -> Callable[[Path], None]:
    """Return an edit that switches the project adapter."""

    def switch(root: Path) -> None:
        replace_project_text(
            root, "sqlbuild_project.toml", f'adapter = "{before}"', f'adapter = "{after}"'
        )

    return switch


def disable_project_reuse(monkeypatch: pytest.MonkeyPatch) -> None:
    """Set the fixture environment but compile every time, so the finer caches are exercised."""

    enable_compile_reuse(monkeypatch)
    monkeypatch.setenv(REUSE_DISABLE_ENV_VAR, "1")


def sql_test_scan_counts(run: CompileReuseRun) -> tuple[int, int]:
    """Return the SQL-test scan store hits and misses of one compile."""

    return run.timings["sql_test_scan_cache_hits"], run.timings["sql_test_scan_cache_misses"]


def native_analysis_counts(run: CompileReuseRun) -> tuple[int, int, int]:
    """Return the analysis cache entry hits, misses and bypasses of one compile."""

    return (
        run.timings["analysis_entry_cache_hits"],
        run.timings["analysis_cache_misses"],
        run.timings["analysis_cache_bypasses"],
    )


def edit_staging_type(root: Path, _monkeypatch: pytest.MonkeyPatch) -> None:
    """Change one staging output type, which reaches the staging model's consumers."""

    staging_type_change(root)


def corrupt_native_analysis_store(root: Path, _monkeypatch: pytest.MonkeyPatch) -> None:
    """Overwrite the stored native model analyses with bytes that are not a store file."""

    path: Path = compiler_cache_directory(root) / NATIVE_ANALYSIS_STORE_FILE_NAME
    assert path.is_file()
    _ = path.write_bytes(b"not a native store")


def edit_sql_test_scan_input(root: Path, _monkeypatch: pytest.MonkeyPatch) -> None:
    """Edit one SQL test file between compiles."""

    stg_orders_test_edit(root)


def edit_unrelated_model(root: Path, _monkeypatch: pytest.MonkeyPatch) -> None:
    """Edit a model no SQL test text contains."""

    fact_comment(root)


def change_adapter_lexical_rules(_root: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    """Give the project adapter different lexical rules that leave this project's SQL unchanged."""

    monkeypatch.setattr(
        DuckDbBackedAdapter,
        "sql_lexical_syntax",
        replace(DuckDbBackedAdapter.sql_lexical_syntax, triple_quoted_strings=True),
    )


def upgrade_native_build(_root: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    """Pretend the native extension was rebuilt from different source."""

    monkeypatch.setattr(native_module, "BUILD_IDENTITY", f"{native_module.BUILD_IDENTITY}-rebuilt")


def corrupt_sql_test_scan_store(root: Path, _monkeypatch: pytest.MonkeyPatch) -> None:
    """Overwrite the stored SQL-test scans with bytes that are not a store file."""

    path: Path = compiler_cache_directory(root) / native_module.SQL_TEST_SCAN_STORE_FILE_NAME
    assert path.is_file()
    _ = path.write_bytes(b"not a native store")


def edit_installed_code(_root: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    """Pretend the installed Python code changed, as a local edit of an editable install does."""

    monkeypatch.setattr(native_module, "compiled_code_identity", lambda: "edited-python-code")


RETIRED_RENDER_FILES: tuple[str, ...] = (
    "0123456789abcdef-0000.render",
    "fedcba9876543210-1111.render",
)


def write_retired_compiler_cache_files(project_dir: Path) -> tuple[Path, ...]:
    """Leave retired fact and reference caches and other slots' render files behind."""

    written: list[Path] = []
    for suffix in RETIRED_CACHE_NAMESPACE_SUFFIXES:
        engine_root: Path = (
            compiler_cache_directory(project_dir).parent
            / f"{COMPILER_CACHE_DIRECTORY_NAME}{suffix}"
        )
        written.extend(
            engine_root / retired_directory / "sql-tests.sqlite3"
            for retired_directory in (
                RETIRED_FACT_CACHE_DIRECTORY_NAME,
                RETIRED_REFERENCE_CACHE_DIRECTORY_NAME,
            )
        )
    written.extend(
        compiler_cache_directory(project_dir) / RETIRED_REUSE_DIRECTORY_NAME / name
        for name in RETIRED_RENDER_FILES
    )
    for path in written:
        path.parent.mkdir(parents=True, exist_ok=True)
        _ = path.write_bytes(b"retired")
    return tuple(written)


def _stored_scan_values_replaced(
    replacement: bytes,
) -> Callable[[Path, pytest.MonkeyPatch], None]:
    def change(root: Path, _monkeypatch: pytest.MonkeyPatch) -> None:
        """Rewrite every stored scan value on disk, keeping a valid store frame and checksum."""

        path: Path = compiler_cache_directory(root) / native_module.SQL_TEST_SCAN_STORE_FILE_NAME
        frame: bytes = path.read_bytes()
        header: bytes = frame[:_STORE_PREFIX_LENGTH]
        body: bytes = frame[_STORE_PREFIX_LENGTH + _STORE_CHECKSUM_LENGTH :]
        fields: list[bytes] = []
        cursor: int = 0
        for _ in range(2):
            text, cursor = _store_bytes(body, cursor)
            fields.append(text)
        generation: bytes = body[cursor : cursor + 8]
        metadata, cursor = _store_bytes(body, cursor + 8)
        count: int = int.from_bytes(body[cursor : cursor + 8], "little")
        cursor += 8
        entries: list[bytes] = []
        for _ in range(count):
            key, cursor = _store_bytes(body, cursor)
            last_used: bytes = body[cursor : cursor + 8]
            _value, cursor = _store_bytes(body, cursor + 8)
            entries.append(_framed(key) + last_used + _framed(replacement))
        assert cursor == len(body) and count > 0
        rewritten: bytes = b"".join(
            (*map(_framed, fields), generation, _framed(metadata), count.to_bytes(8, "little"))
        ) + b"".join(entries)
        _ = path.write_bytes(rewritten)
        checksum: bytes = bytes.fromhex(_native_file_digest(path))
        _ = path.write_bytes(header + checksum + rewritten)

    return change


_STORE_PREFIX_LENGTH: int = 12


def _native_file_digest(path: Path) -> str:
    """The BLAKE3 digest native stores checksum their bodies with, of the file at `path`."""

    digest: str | None = native_module.digest_files([str(path)])[0]
    assert digest is not None
    return digest


_STORE_CHECKSUM_LENGTH: int = 32


def _store_bytes(body: bytes, cursor: int) -> tuple[bytes, int]:
    """One length-prefixed field of a native store body, and the cursor after it."""

    length: int = int.from_bytes(body[cursor : cursor + 8], "little")
    return body[cursor + 8 : cursor + 8 + length], cursor + 8 + length


def _framed(value: bytes) -> bytes:
    return len(value).to_bytes(8, "little") + value


replace_stored_scans_undecodable: Callable[[Path, pytest.MonkeyPatch], None] = (
    _stored_scan_values_replaced(b"{]")
)
replace_stored_scans_misshapen: Callable[[Path, pytest.MonkeyPatch], None] = (
    _stored_scan_values_replaced(b'[{"mode": "model"}]')
)


def keep_invalidation(_monkeypatch: pytest.MonkeyPatch) -> None:
    """Leave every cache invalidation intact."""


def ignore_project_changes(monkeypatch: pytest.MonkeyPatch) -> None:
    """Make compile reuse stop watching the models folder, so a model edit goes unseen."""

    monkeypatch.setattr(
        native_module,
        "COMPILE_REUSE_EXCLUDED_ROOT_DIRECTORIES",
        native_module.COMPILE_REUSE_EXCLUDED_ROOT_DIRECTORIES | {"models"},
    )


EXTERNAL_FLAVOR_MODULE: str = "extflavor"


def write_external_flavor(extlib: Path, value: str) -> None:
    """Write, or rewrite in place, an outside module that a macro imports while rendering."""

    extlib.mkdir(parents=True, exist_ok=True)
    (extlib / f"{EXTERNAL_FLAVOR_MODULE}.py").write_text(f"VALUE = {value!r}\n", encoding="utf-8")


def add_external_flavor_macro(*, project_dir: Path, extlib: Path, value: str) -> dict[str, str]:
    """Make the fact orders render import an outside module; return the compile environment."""

    write_external_flavor(extlib, value)
    write_project_file(
        project_dir,
        "models/marts/_sqlbuild/_macros/python/flavor.py",
        '"""Flavor macros backed by an outside module."""\n\n\n'
        "def flavor() -> str:\n"
        '    """Return the configured flavor literal."""\n'
        f"    import {EXTERNAL_FLAVOR_MODULE}\n\n"
        f"    return {EXTERNAL_FLAVOR_MODULE}.VALUE\n",
    )
    replace_project_text(
        project_dir,
        FACT_ORDERS_MODEL,
        "  o.quantity,\n",
        "  o.quantity,\n  @py.flavor() AS flavor,\n",
    )
    return {"PYTHONPATH": str(extlib), "PYTHONDONTWRITEBYTECODE": "1"}


STORED_FLAVOR_LOG_ENV_VAR: str = "SQB_FLAVOR_CALL_LOG"


def write_external_modules(*, extlib: Path, files: dict[str, str]) -> None:
    """Write, or rewrite in place, outside modules on the compile's import path."""

    for relative_path, contents in files.items():
        path: Path = extlib / relative_path
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(contents, encoding="utf-8")


def add_stored_flavor_macro(
    *, project_dir: Path, extlib: Path, import_name: str, log_path: Path
) -> dict[str, str]:
    """Make fact orders call a logged macro that lazily imports `import_name`; return its env."""

    write_project_file(
        project_dir,
        "models/marts/_sqlbuild/_macros/python/flavor.py",
        '"""Flavor macros backed by an outside module."""\n\n\n'
        "def flavor() -> str:\n"
        '    """Return the configured flavor literal and log the call."""\n'
        "    import os\n\n"
        f"    import {import_name} as source\n\n"
        f'    with open(os.environ["{STORED_FLAVOR_LOG_ENV_VAR}"], "a", encoding="utf-8") as log:\n'
        '        log.write("call\\n")\n'
        "    return source.VALUE\n",
    )
    replace_project_text(
        project_dir,
        FACT_ORDERS_MODEL,
        "  o.quantity,\n",
        "  o.quantity,\n  @py.flavor() AS flavor,\n",
    )
    log_path.write_text("", encoding="utf-8")
    return {
        "PYTHONPATH": str(extlib),
        "PYTHONDONTWRITEBYTECODE": "1",
        STORED_FLAVOR_LOG_ENV_VAR: str(log_path),
    }


def logged_calls(log_path: Path) -> int:
    """Return how many times a logged macro ran."""

    return len(log_path.read_text(encoding="utf-8").splitlines())


def compiled_text(*, run: CompileReuseRun, suffix: str) -> str:
    """Return the compiled artifact whose path ends with the suffix."""

    path: str = next(filter(lambda path: path.endswith(suffix), sorted(run.compiled)))
    return run.compiled[path].decode("utf-8")


def copy_compile_project(*, source: Path, destination: Path) -> Path:
    """Copy a prepared project to a new directory and return the copy."""

    _ = shutil.copytree(source, destination)
    return destination


def store_digests(*, project_dir: Path, stores: tuple[str, ...]) -> dict[str, str]:
    """Hash every file below the given project-relative store directories."""

    files: list[Path] = []
    for store in stores:
        files.extend(filter(Path.is_file, sorted((project_dir / store).rglob("*"))))
    return {
        path.relative_to(project_dir).as_posix(): hashlib.sha256(path.read_bytes()).hexdigest()
        for path in files
    }


_REFERENCE_CALL_PROJECT_TOML: str = (
    'name = "orders"\nadapter = "duckdb"\n\n[connection]\ndatabase = "warehouse.duckdb"\n'
)
_REFERENCE_CALL_SOURCES_YML: str = (
    "sources:\n"
    "  - name: raw_orders\n"
    "    description: Raw orders.\n"
    "    schema: main\n"
    "    table: raw_orders\n"
    "    columns:\n"
    "      - name: order_id\n"
    "        type: INTEGER\n"
    "      - name: amount_cents\n"
    "        type: INTEGER\n"
)
_REFERENCE_CALL_RAW_ORDERS_SQL: str = (
    "CREATE TABLE main.raw_orders AS "
    "SELECT * FROM (VALUES (1, 1200), (2, 1650)) AS t(order_id, amount_cents)"
)


def prepare_reference_call_project(*, tmp_path: Path, staging_from: str, mart_from: str) -> Path:
    """Write an orders project whose models use the given reference calls, with raw data."""

    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="orders",
        repo_files={
            "sqlbuild_project.toml": _REFERENCE_CALL_PROJECT_TOML,
            "sources/raw.yml": _REFERENCE_CALL_SOURCES_YML,
            "models/staging/stg_orders.sql": (
                'MODEL (description "Staged orders.", materialized view);\n\n'
                f"SELECT order_id, amount_cents FROM {staging_from}\n"
            ),
            "models/marts/order_totals.sql": (
                'MODEL (description "Order totals.", materialized table);\n\n'
                "SELECT COUNT(*) AS order_count, SUM(amount_cents) AS total_cents\n"
                f"FROM {mart_from} AS orders\n"
            ),
        },
    )
    seeded: subprocess.CompletedProcess[str] = run_sqb(
        project_dir=project_dir, command=("query", _REFERENCE_CALL_RAW_ORDERS_SQL)
    )
    assert seeded.returncode == 0, seeded.stdout + seeded.stderr
    return project_dir


_MACRO_REFERENCE_CALL_MODELS: tuple[str, ...] = ("customer_orders", "customer_returns")
_DIAGNOSTIC_FIELDS: tuple[str, ...] = (
    "code",
    "message",
    "resource_name",
    "path",
    "line",
    "column",
    "help",
)


class MacroReferenceCallRuns(NamedTuple):
    """Repeated compiles: exit codes, macro executions, and the diagnostics of each run."""

    returncodes: tuple[int, ...]
    logged_calls: tuple[int, ...]
    diagnostics: tuple[tuple[tuple[object, ...], ...], ...]


def prepare_macro_reference_call_project(*, tmp_path: Path) -> Path:
    """Write a project whose models share a macro returning a reference call compile rejects."""

    return prepare_inline_project(
        tmp_path=tmp_path,
        project_name="orders",
        repo_files={
            "sqlbuild_project.toml": _REFERENCE_CALL_PROJECT_TOML,
            "sources/raw.yml": _REFERENCE_CALL_SOURCES_YML,
            "models/staging/customers.sql": (
                'MODEL (description "Customers.", materialized view);\n\n'
                'SELECT order_id AS customer_id FROM __source("raw_orders")\n'
            ),
            "models/marts/_sqlbuild/_macros/python/customer_relation.py": (
                '"""A macro that returns an unquoted reference call."""\n\n'
                "import os\n\n\n"
                "def customer_relation() -> str:\n"
                '    """Return the customers relation, logging each execution."""\n'
                f'    path = os.environ.get("{MACRO_CALL_LOG_ENV_VAR}")\n'
                "    if path:\n"
                '        with open(path, "a", encoding="utf-8") as log:\n'
                '            log.write("customer_relation\\n")\n'
                '    return "__ref(customers)"\n'
            ),
            **{
                f"models/marts/{name}.sql": (
                    f'MODEL (description "{name}.", materialized view);\n\n'
                    "SELECT customer_id FROM @py.customer_relation()\n"
                )
                for name in _MACRO_REFERENCE_CALL_MODELS
            },
        },
    )


def _diagnostic_fields(item: dict[str, object]) -> tuple[object, ...]:
    return tuple(item.get(field) for field in _DIAGNOSTIC_FIELDS)


def macro_reference_call_runs(
    *, project_dir: Path, log_path: Path, runs: int
) -> MacroReferenceCallRuns:
    """Compile `runs` times in fresh processes, keeping the macro call store between runs."""

    env: dict[str, str] = {
        REUSE_DISABLE_ENV_VAR: "1",
        MACRO_CALL_LOG_ENV_VAR: str(log_path),
    }
    returncodes: list[int] = []
    logged_calls: list[int] = []
    diagnostics: list[tuple[tuple[object, ...], ...]] = []
    for _ in range(runs):
        _ = log_path.write_text("", encoding="utf-8")
        compiled: subprocess.CompletedProcess[str] = run_installed_sqb(
            project_dir=project_dir, args=("compile", "--json"), env=env
        )
        returncodes.append(compiled.returncode)
        logged_calls.append(len(log_path.read_text(encoding="utf-8").splitlines()))
        items: list[dict[str, object]] = cast(
            list[dict[str, object]], json.loads(compiled.stdout)["diagnostics"]
        )
        diagnostics.append(tuple(_diagnostic_fields(item) for item in items))
    return MacroReferenceCallRuns(
        returncodes=tuple(returncodes),
        logged_calls=tuple(logged_calls),
        diagnostics=tuple(diagnostics),
    )


STORE_ENVIRONMENT_REGION_VAR: str = "SQB_STORE_TEST_REGION"
STORE_ARGUMENT_ENV_VAR: str = "STORE_TEST_ARGUMENT"
_REUSE_DISABLED_VALUES: dict[bool, str] = {True: "0", False: "1"}
STALE_FLAVOR_EDIT: str = "salty"
_SOUTH_MODEL: str = "models/south/orders_south.sql"
_STORE_EXTERNAL_MODULE: str = "store_flavor_values"
_ZIPPED_FLAVOR_MODULE: str = "zipped_flavor_values"
_STORE_IMPORTED_MODULES: tuple[str, ...] = (
    _STORE_EXTERNAL_MODULE,
    _ZIPPED_FLAVOR_MODULE,
    "macros",
    "macros._label_values",
)
_STORE_PROJECT_EXTRA_FILES: dict[str, str] = {
    "macros/python/context_labels.py": (
        '"""Macros that read the target, the environment and an outside module."""\n\n'
        "import os\n\n\n"
        "def env_region() -> str:\n"
        '    """Return the region from the environment."""\n'
        f'    return repr(os.environ.get("{STORE_ENVIRONMENT_REGION_VAR}", "none"))\n\n\n'
        "def target_label(ctx) -> str:\n"
        '    """Name the compile target."""\n'
        "    return repr(ctx.target_name)\n\n\n"
        "def flavor() -> str:\n"
        '    """Return a value from a module outside the project, imported while rendering."""\n'
        f"    import {_STORE_EXTERNAL_MODULE}\n\n"
        f"    return repr({_STORE_EXTERNAL_MODULE}.VALUE)\n"
    ),
    "macros/python/_label_values.py": (
        'def label_value() -> str:\n    """Return the label literal."""\n    return "\'first\'"\n'
    ),
    "macros/python/labels.py": (
        "from macros.python._label_values import label_value\n\n\n"
        'def label() -> str:\n    """Return the label from a helper module."""\n'
        "    return label_value()\n"
    ),
}


class MacroCallStoreRun(NamedTuple):
    """A compile with the macro call store, its --no-cache reference, and macro executions."""

    incremental: CompileReuseRun
    reference: CompileReuseRun
    logged_calls: int

    @property
    def matches(self) -> bool:
        """Return whether exit code, report, and every compiled artifact are byte-identical."""

        return _edit_outcome(self.incremental) == _edit_outcome(self.reference)


def prepare_macro_call_store_project(*, project_dir: Path, extlib: Path) -> None:
    """Write a macro-heavy project whose macros read vars, target, constants and enums."""

    for relative_path, content in MACRO_BRIDGE_PROJECT_FILES.items():
        write_project_file(
            project_dir, relative_path, content.replace("WHERE id IN @py.generated_join()\n", "")
        )
    for relative_path, content in _STORE_PROJECT_EXTRA_FILES.items():
        write_project_file(project_dir, relative_path, content)
    replace_project_text(
        project_dir,
        "sqlbuild_project.toml",
        '[targets.dev]\nschema = "analytics"\n',
        '[targets.dev]\nschema = "analytics"\n\n[targets.prod]\nschema = "analytics"\n',
    )
    replace_project_text(
        project_dir,
        _SOUTH_MODEL,
        '  description "South orders",\n',
        '  description "South orders",\n  constants (_south_bonus 3),\n',
    )
    replace_project_text(
        project_dir,
        _SOUTH_MODEL,
        "  'quoted @py.cents(1)' AS quoted",
        "  @py.target_label() AS target_label,\n  @py.flavor() AS flavor,\n  @py.label() AS "
        "label,\n"
        "  @py.env_region() AS env_region,\n"
        "  'quoted @py.cents(1)' AS quoted",
    )
    write_store_flavor(extlib=extlib, value="sweet")


def write_store_flavor(*, extlib: Path, value: str) -> None:
    """Write, or rewrite in place, the outside module the flavor macro imports."""

    extlib.mkdir(parents=True, exist_ok=True)
    (extlib / f"{_STORE_EXTERNAL_MODULE}.py").write_text(f"VALUE = {value!r}\n", encoding="utf-8")


def edit_label_helper(project_dir: Path, value: str) -> None:
    """Change the literal the helper module behind the label macro returns."""

    replace_project_text(project_dir, "macros/python/_label_values.py", "'first'", f"'{value}'")


def edit_south_model(project_dir: Path, old: str, new: str) -> None:
    """Replace text in the south orders model."""

    replace_project_text(project_dir, _SOUTH_MODEL, old, new)


def macro_call_store_compile(
    *,
    project_dir: Path,
    extlib: Path,
    log_path: Path,
    project_reuse: bool,
    args: tuple[str, ...] = (),
    extra_env: tuple[tuple[str, str], ...] = (),
) -> MacroCallStoreRun:
    """Compile with the macro call store, count macro executions, then compile with --no-cache."""

    env: dict[str, str] = {
        REUSE_DISABLE_ENV_VAR: _REUSE_DISABLED_VALUES[project_reuse],
        MACRO_CALL_LOG_ENV_VAR: str(log_path),
        "PYTHONPATH": str(extlib),
        "PYTHONDONTWRITEBYTECODE": "1",
        STORE_ARGUMENT_ENV_VAR: "first",
        **dict(extra_env),
    }
    global_args: tuple[str, ...] = ()
    _ = log_path.write_text("", encoding="utf-8")
    incremental: CompileReuseRun = run_reuse_compile(
        project_dir=project_dir, env=env, args=args, global_args=global_args
    )
    logged_calls: int = len(log_path.read_text(encoding="utf-8").splitlines())
    reference: CompileReuseRun = run_reuse_compile(
        project_dir=project_dir, env=env, args=(*args, "--no-cache"), global_args=global_args
    )
    return MacroCallStoreRun(
        incremental=incremental, reference=reference, logged_calls=logged_calls
    )


def freeze_macro_call_store_environment(monkeypatch: pytest.MonkeyPatch) -> None:
    """Break the store key: every project and module state looks like the first one."""

    monkeypatch.setattr(macro_bridge_class, "store_environment", lambda **_kwargs: "frozen")
    monkeypatch.setattr(macro_bridge_class, "unchanged_module_digests", lambda **_kwargs: {})
    monkeypatch.setattr(macro_bridge_class.MacroBridge, "observe_modules", lambda _bridge: True)
    monkeypatch.setattr(call_store_module, "claim_first_compile", lambda: True)


def pretend_fresh_process(monkeypatch: pytest.MonkeyPatch) -> None:
    """Treat this test process as one no compile has run in yet."""

    compiles: Iterator[int] = itertools.count()
    monkeypatch.setattr(call_store_module, "claim_first_compile", lambda: next(compiles) == 0)


def logged_in_process_compile(
    *, project_dir: Path, log_path: Path, capsys: pytest.CaptureFixture[str]
) -> int:
    """Compile in this process and return how many macro calls ran."""

    _ = log_path.write_text("", encoding="utf-8")
    _ = in_process_reuse_run(project_dir=project_dir, capsys=capsys)
    return len(log_path.read_text(encoding="utf-8").splitlines())


def _save_after(monkeypatch: pytest.MonkeyPatch, change: Callable[[], object]) -> None:
    original: Callable[[macro_bridge_class.MacroBridge], None] = (
        macro_bridge_class.MacroBridge.save_store
    )
    pending: Iterator[Callable[[], object]] = iter([change])

    def change_then_save(bridge: macro_bridge_class.MacroBridge) -> None:
        _ = [next_change() for next_change in itertools.islice(pending, 1)]
        original(bridge)

    monkeypatch.setattr(macro_bridge_class.MacroBridge, "save_store", change_then_save)


def recompile_in_process_after_edit(arrangement: StaleStoreArrangement) -> None:
    """Compile twice in one process, editing the flavor module between the compiles."""

    pretend_fresh_process(arrangement.monkeypatch)
    _ = in_process_reuse_run(project_dir=arrangement.project_dir, capsys=arrangement.capsys)
    write_store_flavor(extlib=arrangement.extlib, value=STALE_FLAVOR_EDIT)
    _ = in_process_reuse_run(project_dir=arrangement.project_dir, capsys=arrangement.capsys)


def edit_flavor_while_saving(arrangement: StaleStoreArrangement) -> None:
    """Compile in a fresh process that edits the flavor module just before saving the store."""

    pretend_fresh_process(arrangement.monkeypatch)
    _save_after(
        arrangement.monkeypatch,
        lambda: write_store_flavor(extlib=arrangement.extlib, value=STALE_FLAVOR_EDIT),
    )
    _ = in_process_reuse_run(project_dir=arrangement.project_dir, capsys=arrangement.capsys)


def move_flavor_while_saving(arrangement: StaleStoreArrangement) -> None:
    """Move the loaded flavor away before saving, then write a new one, keeping folder times."""

    module_path: Path = arrangement.extlib / f"{_STORE_EXTERNAL_MODULE}.py"
    folder: os.stat_result = arrangement.extlib.stat()
    pretend_fresh_process(arrangement.monkeypatch)
    _save_after(
        arrangement.monkeypatch, lambda: module_path.rename(module_path.with_suffix(".moved"))
    )
    _ = in_process_reuse_run(project_dir=arrangement.project_dir, capsys=arrangement.capsys)
    write_store_flavor(extlib=arrangement.extlib, value=STALE_FLAVOR_EDIT)
    os.utime(arrangement.extlib, ns=(folder.st_atime_ns, folder.st_mtime_ns))


def rezip_flavor_between_processes(arrangement: StaleStoreArrangement) -> None:
    """Serve the flavor from a zip archive, compile in a process, then rebuild the archive."""

    write_zipped_flavor(extlib=arrangement.extlib, value="sweet")
    _ = fresh_process_compile(project_dir=arrangement.project_dir, extlib=arrangement.extlib)
    write_zipped_flavor(extlib=arrangement.extlib, value=STALE_FLAVOR_EDIT)


def backdate_flavor_between_processes(arrangement: StaleStoreArrangement) -> None:
    """Compile in a process, then replace the flavor keeping its size and modification time."""

    module_path: Path = arrangement.extlib / f"{_STORE_EXTERNAL_MODULE}.py"
    _ = fresh_process_compile(project_dir=arrangement.project_dir, extlib=arrangement.extlib)
    status: os.stat_result = module_path.stat()
    write_store_flavor(extlib=arrangement.extlib, value=STALE_FLAVOR_EDIT)
    os.utime(module_path, ns=(status.st_atime_ns, status.st_mtime_ns))


def write_zipped_flavor(*, extlib: Path, value: str) -> None:
    """Make the flavor module re-export a value from a module inside a zip archive."""

    archive: Path = extlib / "flavors.zip"
    with zipfile.ZipFile(archive, "w") as bundle:
        bundle.writestr(f"{_ZIPPED_FLAVOR_MODULE}.py", f"VALUE = {value!r}\n")
    _ = (extlib / f"{_STORE_EXTERNAL_MODULE}.py").write_text(
        f"import sys\n\nsys.path.insert(0, {str(archive)!r})\n"
        f"from {_ZIPPED_FLAVOR_MODULE} import VALUE\n",
        encoding="utf-8",
    )


def fresh_process_compile(*, project_dir: Path, extlib: Path) -> MacroCallStoreRun:
    """Compile with the macro call store in a new process, then with --no-cache."""

    return macro_call_store_compile(
        project_dir=project_dir,
        extlib=extlib,
        log_path=project_dir.parent / "fresh-calls.log",
        project_reuse=False,
    )


def compile_in_new_process(arrangement: StaleStoreArrangement) -> CompileReuseRun:
    """Compile with the macro call store in a new process."""

    return fresh_process_compile(
        project_dir=arrangement.project_dir, extlib=arrangement.extlib
    ).incremental


def compile_as_new_process_here(arrangement: StaleStoreArrangement) -> CompileReuseRun:
    """Compile in this process as if it had just started, re-importing the flavor modules."""

    forget_store_flavor_module()
    pretend_fresh_process(arrangement.monkeypatch)
    return in_process_reuse_run(project_dir=arrangement.project_dir, capsys=arrangement.capsys)


def uncached_reference_compile(*, project_dir: Path, extlib: Path) -> CompileReuseRun:
    """Compile with --no-cache in a new process, the oracle for stored results."""

    return run_reuse_compile(
        project_dir=project_dir,
        env={"PYTHONPATH": str(extlib)},
        args=("--no-cache",),
    )


def block_proc_reads(extlib: Path) -> None:
    """Make Python file reads under /proc fail in processes importing from `extlib`."""

    _ = (extlib / "sitecustomize.py").write_text(
        "import builtins\nimport io\n\n_open = io.open\n\n\n"
        "def _guarded_open(file, *args, **kwargs):\n"
        "    if str(file).startswith('/proc'):\n"
        "        raise FileNotFoundError(file)\n"
        "    return _open(file, *args, **kwargs)\n\n\n"
        "builtins.open = io.open = _guarded_open\n",
        encoding="utf-8",
    )


def forget_store_flavor_module() -> None:
    """Drop the flavor and project macro helper modules in-process compiles imported."""

    _ = [sys.modules.pop(name, None) for name in _STORE_IMPORTED_MODULES]


def store_files(project_dir: Path) -> dict[str, bytes]:
    """Every macro call store file under the project target folder, by relative path."""

    return {
        path.relative_to(project_dir).as_posix(): path.read_bytes()
        for path in sorted((project_dir / "target").rglob(MACRO_CALL_STORE_FILE_NAME))
    }


class FullMacroCallRuns(NamedTuple):
    """Full compiles: exit codes, macro executions, and macro call stores."""

    returncodes: tuple[int, ...]
    logged_calls: tuple[int, ...]
    store_files: tuple[str, ...]


def full_macro_call_runs(
    *, project_dir: Path, extlib: Path, log_path: Path, runs: int
) -> FullMacroCallRuns:
    """Compile fully `runs` times."""

    env: dict[str, str] = {
        REUSE_DISABLE_ENV_VAR: "1",
        MACRO_CALL_LOG_ENV_VAR: str(log_path),
        "PYTHONPATH": str(extlib),
        "PYTHONDONTWRITEBYTECODE": "1",
    }
    returncodes: list[int] = []
    logged_calls: list[int] = []
    for _ in range(runs):
        _ = log_path.write_text("", encoding="utf-8")
        returncodes.append(run_reuse_compile(project_dir=project_dir, env=env).returncode)
        logged_calls.append(len(log_path.read_text(encoding="utf-8").splitlines()))
    return FullMacroCallRuns(
        returncodes=tuple(returncodes),
        logged_calls=tuple(logged_calls),
        store_files=tuple(
            sorted(
                path.relative_to(project_dir).as_posix()
                for path in (project_dir / "target").rglob(MACRO_CALL_STORE_FILE_NAME)
            )
        ),
    )


def seam_recording_compile(
    *,
    project_dir: Path,
    monkeypatch: pytest.MonkeyPatch,
    capsys: pytest.CaptureFixture[str],
) -> CompileReuseRun:
    """Compile in this process with fresh native stage counters."""

    with monkeypatch.context() as patch:
        for name, value in {
            **COMPILE_REUSE_ENV,
            REUSE_DISABLE_ENV_VAR: "1",
        }.items():
            patch.setenv(name, value)
        _ = native_module.compile_stage_work_info(reset=True)
        return in_process_reuse_run(project_dir=project_dir, capsys=capsys)


class NativeTypeAnswers(NamedTuple):
    """What the native type system answered in one compile: each Python-side normalization,
    and contract results that compared at least one typed column."""

    normalized: list[bool]
    typed_comparisons: int


def type_system_compile(
    *,
    project_dir: Path,
    monkeypatch: pytest.MonkeyPatch,
    capsys: pytest.CaptureFixture[str],
) -> tuple[CompileReuseRun, NativeTypeAnswers]:
    """Compile in this process; return the run and what native types answered."""

    normalized: list[bool] = []
    normalize: Callable[[str, str], object] = native_module.normalize_type

    def recorded(type_sql: str, dialect: str) -> object:
        result: object = normalize(type_sql, dialect)
        normalized.append(result is not None)
        return result

    type_normalization.normalize_type.cache_clear()
    with monkeypatch.context() as patch:
        patch.setattr(native_module, "normalize_type", recorded)
        for name, value in {REUSE_DISABLE_ENV_VAR: "1"}.items():
            patch.setenv(name, value)
        run: CompileReuseRun = in_process_reuse_run(project_dir=project_dir, capsys=capsys)
    type_normalization.normalize_type.cache_clear()
    adapter: DuckDbAdapter = DuckDbAdapter()
    project: CompiledProjectView = native_module.build_project_graph(
        discovered_inputs=native_module.discover_project_inputs(project_dir=project_dir),
        adapter=adapter,
    ).project
    result: ContractValidationResult = native_module.evaluate_model_contracts(
        project=project, dialect=adapter.sql_analysis_dialect()
    )
    return run, NativeTypeAnswers(
        normalized=normalized,
        typed_comparisons=contract_statuses(project=project, result=result)["typed_comparisons"],
    )


def lifecycle_error_type(*, project_dir: Path, monkeypatch: pytest.MonkeyPatch) -> str:
    """Compile in this process; return the failed invocation's error type."""

    published: list[LifecycleEvent] = []
    with monkeypatch.context() as patch:
        patch.setenv(REUSE_DISABLE_ENV_VAR, "1")
        patch.setattr(
            EventDispatcher,
            "publish_lifecycle",
            lambda _dispatcher, event: published.append(event),
        )
        with redirect_stdout(StringIO()):
            _ = main(["--project-dir", str(project_dir), "--no-color", "compile"])
    return str(published[-1].payload.get("error_type"))


def fallback_free_preview_compile(
    *,
    project_dir: Path,
    fallbacks: tuple[tuple[ModuleType, str], ...],
    monkeypatch: pytest.MonkeyPatch,
    capsys: pytest.CaptureFixture[str],
) -> tuple[CompileReuseRun, list[str]]:
    """Compile in this process; return the run and fallbacks it called."""

    called: list[str] = []

    def recorder(name: str) -> Callable[..., object]:
        def fallback(**_kwargs: object) -> object:
            called.append(name)
            raise AssertionError(name)

        return fallback

    with monkeypatch.context() as patch:
        for module, name in fallbacks:
            patch.setattr(module, name, recorder(name))
        for variable, value in {
            REUSE_DISABLE_ENV_VAR: "1",
        }.items():
            patch.setenv(variable, value)
        run: CompileReuseRun = in_process_reuse_run(project_dir=project_dir, capsys=capsys)
    return run, called


def write_counted_error_project(*, project_dir: Path, files: dict[str, str]) -> None:
    """Write the failure base project with `files` added or replaced into a fresh directory."""

    shutil.rmtree(project_dir, ignore_errors=True)
    for relative_path, contents in {**FAILURE_BASE_FILES, **files}.items():
        write_project_file(project_dir, relative_path, contents)


def compile_with_binding_error(project_dir: Path) -> dict[str, Any]:
    """Compile with the analysis cache and return the JSON payload of the expected failure."""

    result: subprocess.CompletedProcess[str] = run_sqb(
        project_dir=project_dir, command=("--no-color", "compile", "--json")
    )
    assert result.returncode == 1, result.stdout + result.stderr
    payload: dict[str, Any] = json.loads(result.stdout)
    return payload


_SLOW_LINT_MACRO: str = (
    "import os\n"
    "import time\n\n\n"
    "def slow_total(amount: str) -> str:\n"
    '    """Return the amount, pausing for the macro tests that only SQL lint expands."""\n'
    "    if amount == 'model_amount':\n"
    "        time.sleep(2.0)\n"
    "    if amount == 'slow_amount':\n"
    "        with open(os.environ['SLOW_MACRO_MARKER'], 'a', encoding='utf-8') as marker:\n"
    "            marker.write('expanding\\n')\n"
    "        time.sleep(float(os.environ['SLOW_MACRO_SECONDS']))\n"
    "    return amount\n"
)


def slow_lint_model_files(*, model_count: int) -> dict[str, str]:
    """A macro used by a model, a function, and macro tests that pause when linted."""

    return {
        "macros/python/slow.py": _SLOW_LINT_MACRO,
        "functions/sql/slow_amount.sql": (
            'FUNCTION (description "Slow amount", arguments (p_amount INTEGER), returns INTEGER);\n'
            'SELECT @py.slow_total("p_amount")\n'
        ),
        "models/slow_total.sql": (
            'MODEL (description "Slow total");\n'
            'SELECT @py.slow_total("model_amount") AS amount\n'
            "FROM (SELECT 5 AS model_amount) AS input_values\n"
        ),
        **{
            f"tests/slow_total_{index}.sql": (
                f'TEST (mode macro, name "slow_total_{index}");\n\n'
                "WITH\n"
                "input_values AS (SELECT 5 AS slow_amount),\n"
                "__macro_actual__ AS (\n"
                '  SELECT @py.slow_total("slow_amount") AS amount FROM input_values\n'
                "),\n"
                "__macro_expected__ AS (SELECT 5 AS amount)\n"
                "SELECT 1\n"
            )
            for index in range(model_count)
        },
    }


def restore_default_interrupt() -> None:
    """Give a spawned command default Ctrl-C handling even when its parent ignores SIGINT."""

    _ = signal.signal(signal.SIGINT, signal.SIG_DFL)


def json_diagnostic_codes(stdout: str) -> tuple[str, ...]:
    """Return the diagnostic codes of a compile JSON document; empty stdout has none."""

    document: dict[str, Any] = json.loads(stdout or "{}")
    return tuple(str(diagnostic["code"]) for diagnostic in document.get("diagnostics", ()))


def run_delta_compile(
    *, project_dir: Path, args: tuple[str, ...], env: dict[str, str]
) -> DeltaCompileOutcome:
    """Compile in a fresh process with compile reuse enabled and the given extra environment."""

    result: subprocess.CompletedProcess[str] = run_installed_sqb(
        project_dir=project_dir, args=args, env={REUSE_DISABLE_ENV_VAR: "0", **env}
    )
    return DeltaCompileOutcome(
        returncode=result.returncode, stdout=result.stdout, stderr=result.stderr
    )


def store_shaped_project(
    *,
    project_dir: Path,
    files: tuple[tuple[str, str], ...],
    stored_edits: tuple[tuple[str, str, str], ...],
) -> None:
    """Add `files` to the project, apply `stored_edits` and compile once to store it."""

    for relative_path, contents in files:
        write_project_file(project_dir, relative_path, contents)
    for relative_path, old, new in stored_edits:
        replace_project_text(project_dir, relative_path, old, new)
    stored: DeltaCompileOutcome = run_delta_compile(
        project_dir=project_dir, args=("compile",), env={}
    )
    assert "Traceback" not in stored.stderr, stored.stderr[-2000:]


def run_full_compile(project_dir: Path, *, args: tuple[str, ...]) -> DeltaCompileOutcome:
    """Compile the project at `project_dir` in full, with delta compiles off."""

    return run_delta_compile(
        project_dir=project_dir, args=args, env={DELTA_COMPILE_DISABLE_ENV_VAR: "1"}
    )


def without_compile_timings(*, report: str) -> str:
    """A JSON report without its compile timings, which differ between compiles."""

    return _COMPILE_TIMINGS_PATTERN.sub("", report)


def run_benchmark_generator(
    *, generator: Path, arguments: tuple[str, ...]
) -> subprocess.CompletedProcess[str]:
    """Run a benchmark project generator script with this interpreter and capture its output."""

    return subprocess.run(
        (sys.executable, str(generator), *arguments),
        capture_output=True,
        text=True,
        check=False,
        timeout=300,
    )


def dialect_syntax_model(description: str, sql: str, config: str = "") -> str:
    """Build a model file with a description, optional extra header config and its query."""

    return f"MODEL (description '{description}'{config});\n{sql}\n"


def dialect_ignored_rules_entry(codes: tuple[str, ...]) -> str:
    """A scoped ignore of `codes` for cases whose SQL shape is the parser input under test."""

    ignored: str = ", ".join(f'"{code}"' for code in codes)
    return (
        f'\n[[rules.rule_ignores]]\nrules = [{ignored}]\npaths = ["models/**"]\n'
        'reason = "The case covers parsing this SQL shape."\n'
    )


def append_to_compiled_sql_tests(project_dir: Path) -> None:
    """Append a comment to every compiled SQL test artifact, changing its size."""

    for artifact in sorted((project_dir / "target/compiled/tests").rglob("*.sql")):
        _ = artifact.write_text(artifact.read_text() + "-- changed\n")


def flip_compiled_sql_test_bytes(project_dir: Path) -> None:
    """Swap one character of every compiled SQL test artifact, keeping its size."""

    for artifact in sorted((project_dir / "target/compiled/tests").rglob("*.sql")):
        _ = artifact.write_text(artifact.read_text().replace("SELECT", "select", 1))
