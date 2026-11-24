"""Measure diverse cached compiler workloads against uncached semantic oracles."""

from itertools import filterfalse
from operator import attrgetter
from pathlib import Path

from scripts.cold_compile_performance._helpers.dense_project import dense_model_name
from scripts.cold_compile_performance._helpers.varied_project import write_varied_compile_project
from scripts.cold_compile_performance.exceptions import CompileBenchmarkFixtureError
from sqlbuild._native import (
    Rule,
    RulesConfig,
    build_rule_catalogue,
    load_rules_config,
    select_catalogue_rules,
)
from tests.e2e.src.sqlbuild.cli.commands.main.compile._test_types import VariedCompileCacheTestCase
from tests.e2e.src.sqlbuild.cli.commands.main.compile.helpers import (
    _RETAINED_BENCHMARK_PAYLOAD_KEYS,
    FreshProcessCompileBenchmarkResult,
    _run_fresh_process_compile_benchmark,
)


def run_varied_cache_benchmark(
    *, project_dir: Path, test_case: VariedCompileCacheTestCase
) -> dict[str, FreshProcessCompileBenchmarkResult]:
    write_varied_compile_project(project_dir=project_dir, model_count=test_case.model_count)
    _assert_complete_rules(project_dir)
    measurements: dict[str, FreshProcessCompileBenchmarkResult] = {}
    measurements["cold"] = _measure(
        project_dir=project_dir,
        label="cold",
        budget=test_case.expected_cold_max_seconds,
        retained_payload_keys=(*_RETAINED_BENCHMARK_PAYLOAD_KEYS, "resources"),
    )
    measurements["warm"] = _measure(
        project_dir=project_dir, label="warm", budget=test_case.expected_warm_max_seconds
    )
    leaf: Path = next(
        (project_dir / "sql" / "models").rglob(f"{dense_model_name(test_case.model_count - 1)}.sql")
    )
    _edit_model(leaf)
    measurements.update(_measure_edit(project_dir=project_dir, label="leaf", test_case=test_case))
    shared: Path = next((project_dir / "sql" / "models").rglob(f"{dense_model_name(0)}.sql"))
    _edit_model(shared)
    measurements.update(_measure_edit(project_dir=project_dir, label="shared", test_case=test_case))
    macro: Path = next((project_dir / "sql" / "models").rglob("offset00000.py"))
    contents: str = macro.read_text(encoding="utf-8")
    if contents.count('return f"({expression} + 1)"') != 1:
        raise CompileBenchmarkFixtureError("Expected one editable macro expression")
    macro.write_text(
        contents.replace('return f"({expression} + 1)"', 'return f"({expression} + 2)"'),
        encoding="utf-8",
    )
    measurements.update(_measure_edit(project_dir=project_dir, label="macro", test_case=test_case))
    return measurements


def _assert_complete_rules(project_dir: Path) -> None:
    config: RulesConfig = load_rules_config(project_dir=project_dir)
    catalogue: tuple[Rule, ...] = build_rule_catalogue(
        config=config, project_dir=project_dir, include_custom=True
    )
    selected: tuple[Rule, ...] = select_catalogue_rules(
        catalogue=catalogue, config=config, project_dir=project_dir
    )
    builtin_codes: set[str] = {rule.code for rule in filterfalse(attrgetter("custom"), catalogue)}
    selected_codes: set[str] = {rule.code for rule in filterfalse(attrgetter("custom"), selected)}
    if (
        selected_codes != builtin_codes
        or sum(rule.custom for rule in selected) != 1
        or config.ignore
        or config.rule_exceptions
        or config.rule_ignores
        or config.thresholds
    ):
        raise CompileBenchmarkFixtureError(
            "Expected all built-in rules and one unrelaxed custom rule"
        )


def _edit_model(path: Path) -> None:
    contents: str = path.read_text(encoding="utf-8")
    if contents.count("CAST(id AS INTEGER)") != 1:
        raise CompileBenchmarkFixtureError("Expected one editable model projection")
    path.write_text(
        contents.replace("CAST(id AS INTEGER)", "CAST(COALESCE(id, id) AS INTEGER)"),
        encoding="utf-8",
    )


def _measure_edit(
    *,
    project_dir: Path,
    label: str,
    test_case: VariedCompileCacheTestCase,
) -> dict[str, FreshProcessCompileBenchmarkResult]:
    measurements: dict[str, FreshProcessCompileBenchmarkResult] = {}
    measurements[label] = _measure(
        project_dir=project_dir, label=label, budget=test_case.expected_edit_max_seconds
    )
    measurements[f"after_{label}"] = _measure(
        project_dir=project_dir, label=f"after_{label}", budget=test_case.expected_warm_max_seconds
    )
    config: Path = project_dir / "sqlbuild_project.toml"
    original: bytes = config.read_bytes()
    try:
        config.write_bytes(original + b"\n[rules.cache]\nenabled = false\n")
        measurements[f"oracle_{label}"] = _run_fresh_process_compile_benchmark(
            project_dir=project_dir,
            label=f"oracle-{label}",
            expected_max_wall_seconds=test_case.expected_cold_max_seconds,
            compile_args=("--no-cache",),
        )
    finally:
        config.write_bytes(original)
    return measurements


def _measure(
    *,
    project_dir: Path,
    label: str,
    budget: float,
    retained_payload_keys: tuple[str, ...] = _RETAINED_BENCHMARK_PAYLOAD_KEYS,
) -> FreshProcessCompileBenchmarkResult:
    return _run_fresh_process_compile_benchmark(
        project_dir=project_dir,
        label=f"varied-{label}",
        expected_max_wall_seconds=budget,
        compile_args=(),
        retained_payload_keys=retained_payload_keys,
    )
