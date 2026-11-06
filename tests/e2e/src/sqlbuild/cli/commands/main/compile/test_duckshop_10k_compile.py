"""Fresh-process guard that the generated DuckShop 10K project is reproducible and compiles cleanly.

The guard runs in the dense 10,000-model compile job, under the same 4 GiB memory limit, because a
cold compile of ten thousand models is too slow for the ordinary E2E shards.
"""

import subprocess
from pathlib import Path
from typing import cast

import pytest

from scripts.cold_compile_performance.main.require_compile_memory_scope import (
    require_compile_memory_scope,
)
from tests.e2e.src.sqlbuild.cli.commands.main.compile._test_types import (
    GeneratedBenchmarkCompileTestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.main.compile.helpers import (
    FreshProcessCompileBenchmarkResult,
    _run_fresh_process_compile_benchmark,
    run_benchmark_generator,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import REPO_ROOT

_GIB: int = 1024 * 1024 * 1024


@pytest.mark.performance
@pytest.mark.cold_compile_performance
@pytest.mark.parametrize(
    "test_case",
    (
        GeneratedBenchmarkCompileTestCase(
            "duckshop_models_10000_generated",
            REPO_ROOT / "benchmarks" / "duckshop-10k" / "generate.py",
            9_900,
            60.0,
            3 * _GIB,
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_generated_duckshop_10k_when_compiling_cold_then_matches_checksum_and_is_clean(
    test_case: GeneratedBenchmarkCompileTestCase, tmp_path: Path
) -> None:
    require_compile_memory_scope()
    project_dir: Path = tmp_path / "duckshop_10k"

    generated: subprocess.CompletedProcess[str] = run_benchmark_generator(
        generator=test_case.generator, arguments=("--out", str(project_dir))
    )
    assert generated.returncode == 0, generated.stdout + generated.stderr
    verified: subprocess.CompletedProcess[str] = run_benchmark_generator(
        generator=test_case.generator, arguments=("--verify", str(project_dir))
    )
    assert verified.returncode == 0, verified.stdout + verified.stderr

    result: FreshProcessCompileBenchmarkResult = _run_fresh_process_compile_benchmark(
        project_dir=project_dir,
        label="duckshop-10k",
        expected_max_wall_seconds=test_case.expected_max_wall_seconds,
        compile_args=("--no-cache",),
    )
    summary: dict[str, int] = cast(dict[str, int], result.payload["summary"])
    print(
        f"duckshop 10k compile summary={summary} wall={result.elapsed_seconds:.2f}s "
        f"cpu={result.cpu_seconds:.2f}s peak_rss_mib={result.peak_rss_bytes // (1024 * 1024)}",
        flush=True,
    )
    assert result.payload["diagnostics"] == []
    assert result.payload["has_errors"] is False
    assert summary["errors"] == 0
    assert summary["warnings"] == 0
    assert summary["models"] >= test_case.expected_min_models
    assert summary["tests"] > 0
    assert result.elapsed_seconds < test_case.expected_max_wall_seconds
    assert result.peak_rss_bytes < test_case.expected_max_rss_bytes
