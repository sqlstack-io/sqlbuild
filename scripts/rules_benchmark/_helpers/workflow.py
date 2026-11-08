"""Rules benchmark project construction and scenario workflow."""

from __future__ import annotations

import importlib.metadata
import json
import os
import platform
import re
import shutil
import statistics
import subprocess
import sys
import tempfile
import time
from collections import defaultdict
from collections.abc import Callable
from dataclasses import dataclass
from pathlib import Path

from scripts.rules_benchmark._helpers.custom_rules import write_custom_rules
from scripts.rules_benchmark.constants import (
    CI_DEFAULT_MAX_SECONDS,
    CI_MODEL_COUNTS,
    CI_PRIMARY_MODEL_COUNT,
    CI_SCALE_MODEL_COUNT,
    CI_STRESS_RULE_COUNT,
    CI_STRESS_WARM_ITERATIONS,
    NON_CACHEABLE_REJECTION,
    SQL_SUFFIX,
)
from scripts.rules_benchmark.exceptions import RulesBenchmarkError
from scripts.rules_benchmark.models import BenchmarkResult
from tests.e2e.src.sqlbuild.cli.commands.main.compile.helpers import (
    write_layered_production_compile_project,
)

_COMPILE_TIMEOUT_SECONDS: int = 120
_CI_THRESHOLDS_PATH: Path = Path(__file__).parents[1] / "ci_thresholds.json"
_GNU_TIME_PATH: Path = Path("/usr/bin/time")
_PEAK_RSS_MARKER: str = "__SQLBUILD_BENCHMARK_PEAK_RSS_KIB__="
_NON_CACHEABLE_PROBE_RULE: str = """
import pathlib


@rule(
    code="XSQBRB999",
    slug="non-cacheable-probe",
    message="Non-cacheable probe observed the rules directory",
    remediation="Remove the non-cacheable probe.",
    enabled_by_default=True,
)
def non_cacheable_probe(*, model: Model, ctx: RuleContext) -> list[Finding]:
    listed = pathlib.os.listdir(pathlib.Path(__file__).parent)
    return [ctx.finding(subject=model)] * int(not listed)
"""


@dataclass(frozen=True)
class _BenchmarkProcessResult:
    returncode: int
    stdout: str
    stderr: str
    peak_rss_bytes: int | None


def run_benchmark(*, model_count: int, iterations: int, output: Path | None) -> int:
    """Generate a representative project, execute scenarios, and render measurements."""
    with tempfile.TemporaryDirectory(prefix="sqlbuild-rules-benchmark-") as temporary:
        project_dir: Path = Path(temporary) / "project"
        _write_project(project_dir=project_dir, model_count=model_count)
        results: tuple[BenchmarkResult, ...] = _run_scenarios(
            project_dir=project_dir, model_count=model_count, iterations=iterations
        )
        workload: dict[str, int] = _workload_counts(project_dir=project_dir)
    payload: dict[str, object] = {
        "hardware": _hardware_payload(),
        "iterations": iterations,
        "model_count": model_count,
        "custom_rule_count": 20,
        "multi_edit_model_count": _multi_edit_model_count(model_count),
        "versions": _version_payload(),
        "workload": workload,
        "scenarios": [_result_payload(result=result) for result in results],
    }
    return _write_benchmark_payload(payload=payload, output=output)


def run_rule_count_benchmark(
    *, model_count: int, iterations: int, rule_counts: tuple[int, ...], output: Path | None
) -> int:
    """Measure custom-rule count scaling against one stable generated project."""

    with tempfile.TemporaryDirectory(prefix="sqlbuild-rule-count-benchmark-") as temporary:
        project_dir: Path = Path(temporary) / "project"
        _write_project(project_dir=project_dir, model_count=model_count)
        _ = _invoke(project_dir=project_dir)
        profiles: list[dict[str, object]] = []
        _write_rule_count_checkpoint(
            output=output,
            model_count=model_count,
            iterations=iterations,
            rule_counts=rule_counts,
            profiles=profiles,
            complete=False,
        )
        for rule_count in rule_counts:
            write_custom_rules(project_dir=project_dir, rule_count=rule_count)
            cold: BenchmarkResult = _measure(
                scenario="cold",
                project_dir=project_dir,
                iterations=iterations,
                mutate=lambda iteration: _clear_cache(project_dir),
            )
            _ = _invoke(project_dir=project_dir)
            warm: BenchmarkResult = _measure(
                scenario="unchanged_warm",
                project_dir=project_dir,
                iterations=iterations,
                mutate=lambda iteration: None,
            )
            multi_edit: BenchmarkResult = _measure(
                scenario="multi_model_edit",
                project_dir=project_dir,
                iterations=iterations,
                mutate=lambda iteration: _append_model_markers(
                    project_dir=project_dir,
                    model_count=model_count,
                    edit_count=_multi_edit_model_count(model_count),
                    iteration=iteration,
                ),
            )
            rule_edit: BenchmarkResult = _measure(
                scenario="custom_rule_source_edit",
                project_dir=project_dir,
                iterations=iterations,
                mutate=lambda iteration: _mutate_custom_rule(
                    path=project_dir / "rules" / "benchmark_rules.py", iteration=iteration
                ),
            )
            profiles.append(
                {
                    "custom_rule_count": rule_count,
                    "cache_bytes": _rules_cache_bytes(project_dir),
                    "scenarios": [
                        _result_payload(result=result)
                        for result in (cold, warm, multi_edit, rule_edit)
                    ],
                }
            )
            _write_rule_count_checkpoint(
                output=output,
                model_count=model_count,
                iterations=iterations,
                rule_counts=rule_counts,
                profiles=profiles,
                complete=False,
            )
        workload: dict[str, int] = _workload_counts(project_dir=project_dir)
    return _write_benchmark_payload(
        payload={
            "benchmark": "custom_rule_count_scaling",
            "complete": True,
            "hardware": _hardware_payload(),
            "iterations": iterations,
            "model_count": model_count,
            "multi_edit_model_count": _multi_edit_model_count(model_count),
            "rule_counts": rule_counts,
            "versions": _version_payload(),
            "workload": workload,
            "profiles": profiles,
        },
        output=output,
    )


def run_ci_benchmark(
    *, output: Path, summary_output: Path, max_seconds: int = CI_DEFAULT_MAX_SECONDS
) -> int:
    """Run the bounded required-CI Rules performance profile."""

    started: float = time.perf_counter()
    profiles: list[dict[str, object]] = []
    with tempfile.TemporaryDirectory(prefix="sqlbuild-rules-ci-benchmark-") as temporary:
        root: Path = Path(temporary)
        for model_count in CI_MODEL_COUNTS:
            project_dir: Path = root / f"project-{model_count}"
            _write_project(project_dir=project_dir, model_count=model_count)
            profiles.append(
                _ci_twenty_rule_profile(
                    project_dir=project_dir,
                    model_count=model_count,
                )
            )
            if model_count >= CI_PRIMARY_MODEL_COUNT:
                profiles.append(
                    _ci_hundred_rule_profile(
                        project_dir=project_dir,
                        model_count=model_count,
                    )
                )
    elapsed_seconds: float = time.perf_counter() - started
    benchmark_seconds: float = sum(
        _number(profile.get("benchmark_seconds")) for profile in profiles
    )
    guard_failures: list[str] = []
    for profile in profiles:
        raw_failures: object = profile.get("guard_failures")
        if isinstance(raw_failures, list | tuple):
            guard_failures.extend(
                str(failure) for failure in raw_failures if isinstance(failure, str)
            )
    payload: dict[str, object] = {
        "benchmark": "required_ci_rules_performance",
        "benchmark_schema_version": 1,
        "complete": True,
        "elapsed_seconds": elapsed_seconds,
        "benchmark_seconds": benchmark_seconds,
        "max_seconds": max_seconds,
        "within_time_budget": benchmark_seconds <= max_seconds,
        "guard_failures": guard_failures,
        "thresholds": _ci_thresholds(),
        "hardware": _hardware_payload(),
        "versions": _version_payload(),
        "profiles": profiles,
    }
    _ = _write_benchmark_payload(payload=payload, output=output)
    summary_output.write_text(_ci_summary(payload=payload), encoding="utf-8")
    return 0 if benchmark_seconds <= max_seconds and not guard_failures else 1


def _ci_twenty_rule_profile(*, project_dir: Path, model_count: int) -> dict[str, object]:
    results: list[BenchmarkResult] = [
        _measure(
            scenario="cold",
            project_dir=project_dir,
            iterations=1,
            mutate=lambda iteration: _clear_target(project_dir),
        )
    ]
    results.append(
        _measure(
            scenario="unchanged_warm",
            project_dir=project_dir,
            iterations=_ci_warm_iterations(model_count=model_count, rule_count=20),
            mutate=lambda iteration: None,
        )
    )
    results.append(
        _measure(
            scenario="multi_model_edit",
            project_dir=project_dir,
            iterations=1,
            mutate=lambda iteration: _append_model_markers(
                project_dir=project_dir,
                model_count=model_count,
                edit_count=_multi_edit_model_count(model_count),
                iteration=iteration,
            ),
        )
    )
    if model_count == CI_PRIMARY_MODEL_COUNT:
        results.extend(_ci_five_thousand_invalidation_results(project_dir=project_dir))
    return _ci_profile_payload(
        project_dir=project_dir,
        model_count=model_count,
        rule_count=20,
        results=tuple(results),
    )


def _ci_five_thousand_invalidation_results(*, project_dir: Path) -> tuple[BenchmarkResult, ...]:
    return (
        _measure(
            scenario="rules_cold",
            project_dir=project_dir,
            iterations=1,
            mutate=lambda iteration: _clear_cache(project_dir),
        ),
        _measure(
            scenario="sql_test_edit",
            project_dir=project_dir,
            iterations=1,
            mutate=lambda iteration: _append_marker(
                path=project_dir / "tests" / "unit" / "test_group_00000.sql",
                scenario="sql-test",
                iteration=iteration,
            ),
        ),
        _measure(
            scenario="macro_edit",
            project_dir=project_dir,
            iterations=1,
            mutate=lambda iteration: _set_macro_offset(
                path=project_dir / "macros" / "python" / "macro_00000.py",
                iteration=iteration,
            ),
        ),
        _measure(
            scenario="project_config_edit",
            project_dir=project_dir,
            iterations=1,
            mutate=lambda iteration: _set_benchmark_revision(
                path=project_dir / "sqlbuild_project.toml",
                iteration=iteration,
            ),
        ),
        _measure(
            scenario="custom_rule_helper_edit",
            project_dir=project_dir,
            iterations=1,
            mutate=lambda iteration: _append_marker(
                path=project_dir / "rules" / "benchmark_helpers.py",
                scenario="custom-rule-helper",
                iteration=iteration,
            ),
        ),
        _measure(
            scenario="custom_rule_source_edit",
            project_dir=project_dir,
            iterations=1,
            mutate=lambda iteration: _mutate_custom_rule(
                path=project_dir / "rules" / "benchmark_rules.py",
                iteration=iteration,
            ),
        ),
    )


def _ci_hundred_rule_profile(*, project_dir: Path, model_count: int) -> dict[str, object]:
    write_custom_rules(project_dir=project_dir, rule_count=100)
    results: list[BenchmarkResult] = [
        _measure(
            scenario="rules_cold",
            project_dir=project_dir,
            iterations=1,
            mutate=lambda iteration: _clear_cache(project_dir),
        ),
        _measure(
            scenario="unchanged_warm",
            project_dir=project_dir,
            iterations=_ci_warm_iterations(
                model_count=model_count,
                rule_count=CI_STRESS_RULE_COUNT,
            ),
            mutate=lambda iteration: None,
        ),
    ]
    if model_count == CI_PRIMARY_MODEL_COUNT:
        results.extend(
            (
                _measure(
                    scenario="multi_model_edit",
                    project_dir=project_dir,
                    iterations=1,
                    mutate=lambda iteration: _append_model_markers(
                        project_dir=project_dir,
                        model_count=model_count,
                        edit_count=_multi_edit_model_count(model_count),
                        iteration=iteration,
                    ),
                ),
                _measure(
                    scenario="custom_rule_source_edit",
                    project_dir=project_dir,
                    iterations=1,
                    mutate=lambda iteration: _mutate_custom_rule(
                        path=project_dir / "rules" / "benchmark_rules.py",
                        iteration=iteration,
                    ),
                ),
            )
        )
    return _ci_profile_payload(
        project_dir=project_dir,
        model_count=model_count,
        rule_count=100,
        results=tuple(results),
    )


def _ci_profile_payload(
    *,
    project_dir: Path,
    model_count: int,
    rule_count: int,
    results: tuple[BenchmarkResult, ...],
) -> dict[str, object]:
    cache_bytes: int = _rules_cache_bytes(project_dir)
    guard_failures: tuple[str, ...] = (
        *_ci_cache_guard_failures(
            model_count=model_count,
            rule_count=rule_count,
            results=results,
        ),
        *_ci_resource_guard_failures(
            model_count=model_count,
            rule_count=rule_count,
            cache_bytes=cache_bytes,
            results=results,
        ),
    )
    return {
        "model_count": model_count,
        "custom_rule_count": rule_count,
        "multi_edit_model_count": _multi_edit_model_count(model_count),
        "cache_bytes": cache_bytes,
        "workload": _workload_counts(project_dir=project_dir),
        "scenarios": [_result_payload(result=result) for result in results],
        "benchmark_seconds": sum(sum(result.seconds) for result in results),
        "guard_failures": guard_failures,
    }


def _ci_thresholds() -> dict[str, object]:
    payload: object = json.loads(_CI_THRESHOLDS_PATH.read_text(encoding="utf-8"))
    if not isinstance(payload, dict):
        raise RulesBenchmarkError("CI Rules thresholds must be a JSON object")
    return {str(key): value for key, value in payload.items()}


def _ci_resource_guard_failures(
    *,
    model_count: int,
    rule_count: int,
    cache_bytes: int,
    results: tuple[BenchmarkResult, ...],
) -> tuple[str, ...]:
    payload: dict[str, object] = _ci_thresholds()
    raw_profiles: object = payload.get("profiles")
    profiles: dict[str, object] = (
        {str(key): value for key, value in raw_profiles.items()}
        if isinstance(raw_profiles, dict)
        else {}
    )
    raw_profile: object = profiles.get(f"{model_count}:{rule_count}")
    if not isinstance(raw_profile, dict):
        return (f"missing CI threshold profile for {model_count} models / {rule_count} Rules",)
    profile: dict[str, object] = {str(key): value for key, value in raw_profile.items()}
    failures: list[str] = []
    max_cache_bytes: object = profile.get("max_cache_bytes")
    if isinstance(max_cache_bytes, int) and cache_bytes > max_cache_bytes:
        failures.append(
            f"{model_count} models / {rule_count} Rules cache uses {cache_bytes} bytes, "
            f"above {max_cache_bytes}"
        )
    max_peak_rss_bytes: object = profile.get("max_peak_rss_bytes")
    raw_scenarios: object = profile.get("scenarios")
    scenario_limits: dict[str, object] = (
        {str(key): value for key, value in raw_scenarios.items()}
        if isinstance(raw_scenarios, dict)
        else {}
    )
    measured_scenarios: set[str] = {result.scenario for result in results}
    expected_scenarios: set[str] = set(scenario_limits)
    if measured_scenarios != expected_scenarios:
        failures.append(
            f"{model_count} models / {rule_count} Rules scenario contract differs: "
            f"expected {sorted(expected_scenarios)}, got {sorted(measured_scenarios)}"
        )
    for result in results:
        max_seconds: object = scenario_limits.get(result.scenario)
        median_seconds: float = statistics.median(result.seconds)
        if isinstance(max_seconds, int | float) and median_seconds > max_seconds:
            failures.append(
                f"{model_count} models / {rule_count} Rules / {result.scenario} median took "
                f"{median_seconds:.2f}s, above {max_seconds}s"
            )
        if (
            isinstance(max_peak_rss_bytes, int)
            and result.peak_rss_bytes
            and max(result.peak_rss_bytes) > max_peak_rss_bytes
        ):
            failures.append(
                f"{model_count} models / {rule_count} Rules / {result.scenario} used "
                f"{max(result.peak_rss_bytes)} peak RSS bytes, above {max_peak_rss_bytes}"
            )
    return tuple(failures)


def _ci_warm_iterations(*, model_count: int, rule_count: int) -> int:
    if model_count == CI_SCALE_MODEL_COUNT and rule_count == CI_STRESS_RULE_COUNT:
        return CI_STRESS_WARM_ITERATIONS
    return 1


def _ci_cache_guard_failures(
    *, model_count: int, rule_count: int, results: tuple[BenchmarkResult, ...]
) -> tuple[str, ...]:
    failures: list[str] = []
    total: int = model_count * rule_count + 2
    edit_count: int = _multi_edit_model_count(model_count)
    expected_by_scenario: dict[str, tuple[int, int]] = {
        "cold": (0, total),
        "rules_cold": (0, total),
        "unchanged_warm": (total, 0),
        "multi_model_edit": (total - (rule_count * edit_count + 2), rule_count * edit_count + 2),
        "custom_rule_source_edit": (total - (model_count + 1), model_count + 1),
    }
    if model_count == CI_PRIMARY_MODEL_COUNT and rule_count != CI_STRESS_RULE_COUNT:
        expected_by_scenario.update(
            {
                "sql_test_edit": (90_000, 10_002),
                "macro_edit": (99_900, 102),
                "project_config_edit": (2_020, 97_982),
                "custom_rule_helper_edit": (95_001, 5_001),
            }
        )
    for result in results:
        expected: tuple[int, int] | None = expected_by_scenario.get(result.scenario)
        if expected is None:
            continue
        for sample_index, (hits, misses) in enumerate(
            zip(result.cache_hit_samples, result.cache_miss_samples, strict=True),
            start=1,
        ):
            if (hits, misses) != expected:
                failures.append(
                    f"{model_count} models / {rule_count} Rules / {result.scenario} sample "
                    f"{sample_index}: expected cache {expected[0]}/{expected[1]}, got "
                    f"{hits}/{misses}"
                )
    return tuple(failures)


def _ci_summary(*, payload: dict[str, object]) -> str:
    raw_elapsed: object = payload.get("elapsed_seconds")
    elapsed_seconds: float = float(raw_elapsed) if isinstance(raw_elapsed, int | float) else 0.0
    raw_max_seconds: object = payload.get("max_seconds")
    max_seconds: int = raw_max_seconds if isinstance(raw_max_seconds, int) else 0
    benchmark_seconds: float = _number(payload.get("benchmark_seconds"))
    lines: list[str] = [
        "## Rules performance guards",
        "",
        f"Compiler invocations: {benchmark_seconds:.2f}s / {max_seconds}s budget",
        f"Total setup and benchmark wall time: {elapsed_seconds:.2f}s",
        "",
    ]
    raw_failures: object = payload.get("guard_failures")
    if isinstance(raw_failures, list) and raw_failures:
        lines.extend(("", "### Guard failures", ""))
        lines.extend(f"- {failure}" for failure in raw_failures)
        lines.append("")
    lines.extend(
        (
            (
                "| Models | Custom Rules | Scenario | Median | p95 | Cache hits | "
                "Cache misses | Peak RSS |"
            ),
            "|---:|---:|---|---:|---:|---:|---:|---:|",
        )
    )
    profiles: object = payload.get("profiles")
    if not isinstance(profiles, list):
        return "\n".join(lines) + "\n"
    for raw_profile in profiles:
        if not isinstance(raw_profile, dict):
            continue
        profile: dict[str, object] = {str(key): value for key, value in raw_profile.items()}
        model_count: object = profile.get("model_count")
        rule_count: object = profile.get("custom_rule_count")
        scenarios: object = profile.get("scenarios")
        if not isinstance(scenarios, list):
            continue
        for raw_scenario in scenarios:
            if not isinstance(raw_scenario, dict):
                continue
            scenario: dict[str, object] = {str(key): value for key, value in raw_scenario.items()}
            peak: object = scenario.get("peak_rss_bytes")
            peak_values: dict[str, object] = (
                {str(key): value for key, value in peak.items()} if isinstance(peak, dict) else {}
            )
            peak_bytes: object = peak_values.get("p95_bytes")
            peak_label: str = (
                "n/a" if not isinstance(peak_bytes, int) else f"{peak_bytes / 1_000_000:.0f} MB"
            )
            lines.append(
                f"| {model_count} | {rule_count} | {scenario.get('scenario')} | "
                f"{_number(scenario.get('median_seconds')):.2f}s | "
                f"{_number(scenario.get('p95_seconds')):.2f}s | "
                f"{scenario.get('cache_hits')} | {scenario.get('cache_misses')} | "
                f"{peak_label} |"
            )
    return "\n".join(lines) + "\n"


def _number(value: object) -> float:
    return float(value) if isinstance(value, int | float) else 0.0


def _write_project(*, project_dir: Path, model_count: int) -> None:
    write_layered_production_compile_project(
        project_dir=project_dir,
        model_count=model_count,
        source_count=_scaled_count(model_count=model_count, representative_count=232),
        seed_count=_scaled_count(model_count=model_count, representative_count=46),
        function_count=_scaled_count(model_count=model_count, representative_count=23),
        macro_count=_scaled_count(model_count=model_count, representative_count=12),
        test_count=_scaled_count(model_count=model_count, representative_count=130),
        audit_count=_scaled_count(model_count=model_count, representative_count=700),
    )
    write_custom_rules(project_dir=project_dir)
    config: Path = project_dir / "sqlbuild_project.toml"
    config.write_text(
        config.read_text(encoding="utf-8")
        + (
            '\n[rules]\nselect = ["SQLCLEAN001", "XSQBR"]\n'
            "[rules.thresholds]\nmin_custom_rule_test_cases = 0\n"
            "[rules.cache]\nenabled = true\n"
        ),
        encoding="utf-8",
    )


def _run_scenarios(
    *, project_dir: Path, model_count: int, iterations: int
) -> tuple[BenchmarkResult, ...]:
    results: list[BenchmarkResult] = [
        _measure(
            scenario="cold",
            project_dir=project_dir,
            iterations=iterations,
            mutate=lambda iteration: _clear_target(project_dir),
        )
    ]
    _ = _invoke(project_dir=project_dir)
    results.append(
        _measure(
            scenario="rules_cold",
            project_dir=project_dir,
            iterations=iterations,
            mutate=lambda iteration: _clear_cache(project_dir),
        )
    )
    _ = _invoke(project_dir=project_dir)
    results.append(
        _measure(
            scenario="tracked_rule_input_edit",
            project_dir=project_dir,
            iterations=iterations,
            mutate=lambda iteration: _append_marker(
                path=project_dir / "rules" / "benchmark_input.yaml",
                scenario="rule-input",
                iteration=iteration,
            ),
        )
    )
    results.append(
        _measure(
            scenario="unchanged_warm",
            project_dir=project_dir,
            iterations=iterations,
            mutate=lambda iteration: None,
        )
    )
    finding_path: Path = _generated_model_path(project_dir=project_dir, index=model_count - 1)
    _inject_sql_finding(path=finding_path)
    _ = _invoke(project_dir=project_dir, allow_failure=True)
    results.append(
        _measure(
            scenario="failing_rule",
            project_dir=project_dir,
            iterations=iterations,
            mutate=lambda iteration: None,
            allow_failure=True,
        )
    )
    _remove_sql_finding(path=finding_path)
    _ = _invoke(project_dir=project_dir)
    results.append(
        _measure(
            scenario="leaf_edit",
            project_dir=project_dir,
            iterations=iterations,
            mutate=lambda iteration: _append_marker(
                path=_generated_model_path(project_dir=project_dir, index=model_count - 1),
                scenario="leaf",
                iteration=iteration,
            ),
        )
    )
    results.append(
        _measure(
            scenario="multi_model_edit",
            project_dir=project_dir,
            iterations=iterations,
            mutate=lambda iteration: _append_model_markers(
                project_dir=project_dir,
                model_count=model_count,
                edit_count=_multi_edit_model_count(model_count),
                iteration=iteration,
            ),
        )
    )
    results.append(
        _measure(
            scenario="shared_ancestor_edit",
            project_dir=project_dir,
            iterations=iterations,
            mutate=lambda iteration: _append_marker(
                path=_generated_model_path(project_dir=project_dir, index=0),
                scenario="ancestor",
                iteration=iteration,
            ),
        )
    )
    results.append(
        _measure(
            scenario="sql_test_edit",
            project_dir=project_dir,
            iterations=iterations,
            mutate=lambda iteration: _append_marker(
                path=project_dir / "tests" / "unit" / "test_group_00000.sql",
                scenario="sql-test",
                iteration=iteration,
            ),
        )
    )
    results.append(
        _measure(
            scenario="macro_edit",
            project_dir=project_dir,
            iterations=iterations,
            mutate=lambda iteration: _set_macro_offset(
                path=project_dir / "macros" / "python" / "macro_00000.py",
                iteration=iteration,
            ),
        )
    )
    results.append(
        _measure(
            scenario="project_config_edit",
            project_dir=project_dir,
            iterations=iterations,
            mutate=lambda iteration: _set_benchmark_revision(
                path=project_dir / "sqlbuild_project.toml",
                iteration=iteration,
            ),
        )
    )
    results.append(
        _measure(
            scenario="custom_rule_helper_edit",
            project_dir=project_dir,
            iterations=iterations,
            mutate=lambda iteration: _append_marker(
                path=project_dir / "rules" / "benchmark_helpers.py",
                scenario="custom-rule-helper",
                iteration=iteration,
            ),
        )
    )
    results.append(
        _measure(
            scenario="custom_rule_source_edit",
            project_dir=project_dir,
            iterations=iterations,
            mutate=lambda iteration: _mutate_custom_rule(
                path=project_dir / "rules" / "benchmark_rules.py", iteration=iteration
            ),
        )
    )
    config: Path = project_dir / "sqlbuild_project.toml"
    config.write_text(
        config.read_text(encoding="utf-8").replace("enabled = true", "enabled = false"),
        encoding="utf-8",
    )
    results.append(
        _measure(
            scenario="cache_disabled",
            project_dir=project_dir,
            iterations=iterations,
            mutate=lambda iteration: None,
        )
    )
    config.write_text(
        config.read_text(encoding="utf-8").replace("enabled = false", "enabled = true"),
        encoding="utf-8",
    )
    results.append(
        _measure_rejection(
            project_dir=project_dir,
            iterations=iterations,
        )
    )
    return tuple(results)


def _measure(
    *,
    scenario: str,
    project_dir: Path,
    iterations: int,
    mutate: Callable[[int], None],
    allow_failure: bool = False,
) -> BenchmarkResult:
    elapsed: list[float] = []
    core_ms: list[int] = []
    built_in_rules_ms: list[int] = []
    custom_rules_ms: list[int] = []
    timings_ms: dict[str, list[int]] = defaultdict(list)
    peak_rss_bytes: list[int] = []
    cache_hit_samples: list[int] = []
    cache_miss_samples: list[int] = []
    payload: dict[str, object] = {}
    for iteration in range(iterations):
        mutate(iteration)
        started: float = time.perf_counter()
        payload, peak_rss = _invoke(project_dir=project_dir, allow_failure=allow_failure)
        if peak_rss is not None:
            peak_rss_bytes.append(peak_rss)
        elapsed.append(time.perf_counter() - started)
        timings: object = payload.get("compile_timings")
        if not isinstance(timings, dict):
            raise RulesBenchmarkError("Rules benchmark returned no compile timings")
        timing_values: dict[str, object] = {str(key): value for key, value in timings.items()}
        for key, value in timing_values.items():
            if key.endswith("_ms") and isinstance(value, int) and not isinstance(value, bool):
                timings_ms[key].append(value)
        core_ms.append(
            sum(
                _integer(payload=timing_values, key=key)
                for key in ("discover_ms", "graph_ms", "lineage_ms", "contracts_ms")
            )
        )
        built_in_rules_ms.append(_integer(payload=timing_values, key="built_in_rules_ms"))
        custom_rules_ms.append(_integer(payload=timing_values, key="custom_rules_ms"))
        cache_hit_samples.append(_integer(payload=timing_values, key="rule_cache_hits"))
        cache_miss_samples.append(_integer(payload=timing_values, key="rule_cache_misses"))
    summary: object = payload.get("summary")
    timings = payload.get("compile_timings")
    if not isinstance(summary, dict) or not isinstance(timings, dict):
        raise RulesBenchmarkError("Rules benchmark returned incomplete compile output")
    summary_values: dict[str, object] = {str(key): value for key, value in summary.items()}
    timing_values = {str(key): value for key, value in timings.items()}
    return BenchmarkResult(
        scenario=scenario,
        seconds=tuple(elapsed),
        evaluated_models=_integer(payload=summary_values, key="selected_models"),
        cache_hits=_integer(payload=timing_values, key="rule_cache_hits"),
        cache_misses=_integer(payload=timing_values, key="rule_cache_misses"),
        cache_hit_samples=tuple(cache_hit_samples),
        cache_miss_samples=tuple(cache_miss_samples),
        core_ms=tuple(core_ms),
        built_in_rules_ms=tuple(built_in_rules_ms),
        custom_rules_ms=tuple(custom_rules_ms),
        timings_ms={key: tuple(values) for key, values in sorted(timings_ms.items())},
        peak_rss_bytes=tuple(peak_rss_bytes),
    )


def _invoke(
    *, project_dir: Path, allow_failure: bool = False
) -> tuple[dict[str, object], int | None]:
    result: _BenchmarkProcessResult = _run(project_dir)
    accepted_codes: tuple[int, ...] = (0, 1) if allow_failure else (0,)
    if result.returncode not in accepted_codes or not result.stdout:
        raise RulesBenchmarkError(result.stderr or "Rules benchmark produced no output")
    return json.loads(result.stdout), result.peak_rss_bytes


def _run(project_dir: Path) -> _BenchmarkProcessResult:
    command: list[str] = [
        str(Path(sys.executable).with_name("sqb")),
        "--project-dir",
        str(project_dir),
        "--no-color",
        "compile",
        "--json",
    ]
    if _GNU_TIME_PATH.is_file():
        command = [str(_GNU_TIME_PATH), "-f", f"{_PEAK_RSS_MARKER}%M", "--", *command]
    completed: subprocess.CompletedProcess[str] = subprocess.run(
        command,
        check=False,
        capture_output=True,
        text=True,
        timeout=_COMPILE_TIMEOUT_SECONDS,
    )
    stderr, peak_rss_bytes = _extract_peak_rss(completed.stderr)
    return _BenchmarkProcessResult(
        returncode=completed.returncode,
        stdout=completed.stdout,
        stderr=stderr,
        peak_rss_bytes=peak_rss_bytes,
    )


def _extract_peak_rss(stderr: str) -> tuple[str, int | None]:
    lines: list[str] = []
    peak_rss_bytes: int | None = None
    for line in stderr.splitlines():
        if line.startswith(_PEAK_RSS_MARKER):
            raw_value: str = line.removeprefix(_PEAK_RSS_MARKER)
            if raw_value.isdigit():
                peak_rss_bytes = int(raw_value) * 1024
                continue
        lines.append(line)
    cleaned: str = "\n".join(lines)
    if stderr.endswith("\n") and cleaned:
        cleaned += "\n"
    return cleaned, peak_rss_bytes


def _measure_rejection(*, project_dir: Path, iterations: int) -> BenchmarkResult:
    rule_file: Path = project_dir / "rules" / "benchmark_rules.py"
    with rule_file.open("a", encoding="utf-8") as handle:
        handle.write(_NON_CACHEABLE_PROBE_RULE)
    elapsed: list[float] = []
    peak_rss_bytes: list[int] = []
    for _ in range(iterations):
        started: float = time.perf_counter()
        result: _BenchmarkProcessResult = _run(project_dir)
        elapsed.append(time.perf_counter() - started)
        if result.peak_rss_bytes is not None:
            peak_rss_bytes.append(result.peak_rss_bytes)
        detail: str = f"{result.stderr}\n{result.stdout}".strip()
        if result.returncode == 0 or NON_CACHEABLE_REJECTION not in detail:
            raise RulesBenchmarkError(f"non-cacheable custom rule was not rejected: {detail}")
    return BenchmarkResult(
        scenario="non_cacheable_rejection",
        seconds=tuple(elapsed),
        evaluated_models=0,
        cache_hits=0,
        cache_misses=0,
        peak_rss_bytes=tuple(peak_rss_bytes),
        rejected=True,
    )


def _integer(*, payload: dict[str, object], key: str) -> int:
    value: object = payload.get(key)
    if not isinstance(value, int) or isinstance(value, bool):
        raise RulesBenchmarkError(f"Rules benchmark returned invalid {key}")
    return value


def _clear_cache(project_dir: Path) -> None:
    cache: Path = project_dir / "target" / "rules-cache"
    if cache.exists():
        shutil.rmtree(cache)


def _clear_target(project_dir: Path) -> None:
    target: Path = project_dir / "target"
    if target.exists():
        shutil.rmtree(target)


def _append_marker(*, path: Path, scenario: str, iteration: int) -> None:
    prefix: str = "--" if path.suffix == SQL_SUFFIX else "#"
    with path.open("a", encoding="utf-8") as handle:
        handle.write(f"\n{prefix} benchmark {scenario} {iteration}\n")


def _append_model_markers(
    *, project_dir: Path, model_count: int, edit_count: int, iteration: int
) -> None:
    for index in range(model_count - edit_count, model_count):
        _append_marker(
            path=_generated_model_path(project_dir=project_dir, index=index),
            scenario="multi-model",
            iteration=iteration,
        )


def _multi_edit_model_count(model_count: int) -> int:
    return max(1, model_count // 100)


def _mutate_custom_rule(*, path: Path, iteration: int) -> None:
    source: str = path.read_text(encoding="utf-8")
    marker: str = "def check_003(*, model: Model, ctx: RuleContext) -> list[Finding]:\n"
    if marker not in source:
        marker = "def check_001(*, project: Project, ctx: RuleContext) -> list[Finding]:\n"
    if marker not in source:
        raise RulesBenchmarkError("could not locate custom rule implementation marker")
    source = source.replace(marker, f"{marker}    # benchmark source edit {iteration}\n", 1)
    path.write_text(source, encoding="utf-8")


def _set_benchmark_revision(*, path: Path, iteration: int) -> None:
    source: str = path.read_text(encoding="utf-8")
    changed, replacement_count = re.subn(
        r'benchmark_revision = "[0-9]+"',
        f'benchmark_revision = "{iteration + 1}"',
        source,
        count=1,
    )
    if replacement_count != 1:
        raise RulesBenchmarkError(f"could not locate benchmark marker in {path}")
    path.write_text(changed, encoding="utf-8")


def _set_macro_offset(*, path: Path, iteration: int) -> None:
    source: str = path.read_text(encoding="utf-8")
    changed, replacement_count = re.subn(
        r'return f"\(\{expression\} \+ [0-9]+\)"',
        f'return f"({{expression}} + {iteration + 1000})"',
        source,
        count=1,
    )
    if replacement_count != 1:
        raise RulesBenchmarkError(f"could not locate macro benchmark marker in {path}")
    path.write_text(changed, encoding="utf-8")


def _inject_sql_finding(*, path: Path) -> None:
    source: str = path.read_text(encoding="utf-8")
    marker: str = "SELECT\n"
    if marker not in source:
        raise RulesBenchmarkError("could not locate SQL finding marker")
    path.write_text(
        source.replace(marker, f"{marker}  NULL = NULL AS invalid_null_comparison,\n", 1),
        encoding="utf-8",
    )


def _remove_sql_finding(*, path: Path) -> None:
    source: str = path.read_text(encoding="utf-8")
    path.write_text(
        source.replace("  NULL = NULL AS invalid_null_comparison,\n", "", 1),
        encoding="utf-8",
    )


def _result_payload(*, result: BenchmarkResult) -> dict[str, object]:
    ordered: list[float] = sorted(result.seconds)
    p95_index: int = min(len(ordered) - 1, round(0.95 * (len(ordered) - 1)))
    return {
        "scenario": result.scenario,
        "median_seconds": statistics.median(result.seconds),
        "p95_seconds": ordered[p95_index],
        "samples_seconds": result.seconds,
        "evaluated_models": result.evaluated_models,
        "cache_hits": result.cache_hits,
        "cache_misses": result.cache_misses,
        "cache_hits_distribution": _count_distribution(result.cache_hit_samples),
        "cache_misses_distribution": _count_distribution(result.cache_miss_samples),
        "core_ms": _distribution(result.core_ms),
        "built_in_rules_ms": _distribution(result.built_in_rules_ms),
        "custom_rules_ms": _distribution(result.custom_rules_ms),
        "timings_ms": {key: _distribution(values) for key, values in result.timings_ms.items()},
        "peak_rss_bytes": _byte_distribution(result.peak_rss_bytes),
        "rejected": result.rejected,
    }


def _distribution(values: tuple[int, ...]) -> dict[str, object] | None:
    if not values:
        return None
    ordered: list[int] = sorted(values)
    p95_index: int = min(len(ordered) - 1, round(0.95 * (len(ordered) - 1)))
    return {
        "median_ms": statistics.median(values),
        "p95_ms": ordered[p95_index],
        "samples_ms": values,
    }


def _byte_distribution(values: tuple[int, ...]) -> dict[str, object] | None:
    if not values:
        return None
    ordered: list[int] = sorted(values)
    p95_index: int = min(len(ordered) - 1, round(0.95 * (len(ordered) - 1)))
    return {
        "median_bytes": statistics.median(values),
        "p95_bytes": ordered[p95_index],
        "samples_bytes": values,
    }


def _count_distribution(values: tuple[int, ...]) -> dict[str, object] | None:
    if not values:
        return None
    ordered: list[int] = sorted(values)
    p95_index: int = min(len(ordered) - 1, round(0.95 * (len(ordered) - 1)))
    return {
        "median": statistics.median(values),
        "p95": ordered[p95_index],
        "samples": values,
    }


def _cpu_model() -> str:
    cpuinfo: Path = Path("/proc/cpuinfo")
    if cpuinfo.exists():
        for line in cpuinfo.read_text(encoding="utf-8").splitlines():
            if line.startswith("model name"):
                return line.partition(":")[2].strip()
    return platform.processor() or "unknown"


def _memory_bytes() -> int | None:
    meminfo: Path = Path("/proc/meminfo")
    if not meminfo.exists():
        return None
    for line in meminfo.read_text(encoding="utf-8").splitlines():
        if line.startswith("MemTotal:"):
            return int(line.split()[1]) * 1024
    return None


def _hardware_payload() -> dict[str, object]:
    return {
        "platform": platform.platform(),
        "python": sys.version.split()[0],
        "cpu": _cpu_model(),
        "logical_cpus": os.cpu_count(),
        "memory_bytes": _memory_bytes(),
    }


def _version_payload() -> dict[str, str]:
    return {
        "sqlbuild": importlib.metadata.version("sqlbuild"),
    }


def _write_benchmark_payload(*, payload: dict[str, object], output: Path | None) -> int:
    rendered: str = json.dumps(payload, indent=2, sort_keys=True)
    if output is not None:
        output.write_text(rendered + "\n", encoding="utf-8")
    print(rendered)
    return 0


def _write_rule_count_checkpoint(
    *,
    output: Path | None,
    model_count: int,
    iterations: int,
    rule_counts: tuple[int, ...],
    profiles: list[dict[str, object]],
    complete: bool,
) -> None:
    if output is None:
        return
    payload: dict[str, object] = {
        "benchmark": "custom_rule_count_scaling",
        "complete": complete,
        "hardware": _hardware_payload(),
        "iterations": iterations,
        "model_count": model_count,
        "multi_edit_model_count": _multi_edit_model_count(model_count),
        "rule_counts": rule_counts,
        "versions": _version_payload(),
        "profiles": profiles,
    }
    output.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def _rules_cache_bytes(project_dir: Path) -> int:
    cache_dir: Path = project_dir / "target" / "rules-cache"
    return sum(path.stat().st_size for path in cache_dir.rglob("*") if path.is_file())


def _workload_counts(*, project_dir: Path) -> dict[str, int]:
    model_paths: tuple[Path, ...] = tuple((project_dir / "models").rglob("*.sql"))
    test_paths: tuple[Path, ...] = tuple((project_dir / "tests").rglob("*.sql"))
    rule_paths: tuple[Path, ...] = tuple((project_dir / "rules").rglob("*.py"))
    sources_text: str = "\n".join(
        path.read_text(encoding="utf-8") for path in (project_dir / "sources").rglob("*.yml")
    )
    return {
        "models": len(model_paths),
        "sources": sources_text.count("  - name: source_"),
        "seeds": sum(1 for _ in (project_dir / "seeds").rglob("*.csv")),
        "functions": sum(1 for _ in (project_dir / "functions").rglob("*.sql")),
        "macros": sum(1 for _ in (project_dir / "macros").rglob("*.py")),
        "tests": sum(path.read_text(encoding="utf-8").count("TEST (") for path in test_paths),
        "audits": sum(path.read_text(encoding="utf-8").count("audits [") for path in model_paths),
        "hooks": sum(1 for _ in (project_dir / "hooks").rglob("*.sql")),
        "custom_rules": sum(
            path.read_text(encoding="utf-8").count("@rule(") for path in rule_paths
        ),
    }


def _scaled_count(*, model_count: int, representative_count: int) -> int:
    return max(1, round(model_count * representative_count / 976))


def _generated_model_path(*, project_dir: Path, index: int) -> Path:
    matches: tuple[Path, ...] = tuple((project_dir / "models").rglob(f"model_{index:05d}.sql"))
    if len(matches) != 1:
        raise RulesBenchmarkError(f"expected one generated model path for index {index}")
    return matches[0]
