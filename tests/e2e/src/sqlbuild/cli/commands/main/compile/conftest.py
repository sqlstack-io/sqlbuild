"""Synthetic playground fixture for semantic compilation regression cases."""

import subprocess
from pathlib import Path

import pytest

from scripts.cold_compile_performance.main.assert_required_cgroup_memory_limit import (
    assert_required_cgroup_memory_limit,
)
from tests.e2e.src.sqlbuild.cli.commands.main.compile.helpers import (
    BuiltBenchmark,
    InspectionCommandMeasurement,
    prepare_build_benchmark_project,
    prepare_compile_cache_invalidation_project,
    prepare_compile_reuse_project,
    prepare_inspection_benchmark_project,
    run_fresh_process_inspection_command,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import run_sqb


@pytest.fixture(scope="session")
def semantic_playground(tmp_path_factory: pytest.TempPathFactory) -> Path:
    root: Path = tmp_path_factory.mktemp("semantic_playground")
    project: Path = root / "orders_project"
    result: subprocess.CompletedProcess[str] = run_sqb(
        project_dir=root, command=("playground", str(project))
    )
    assert result.returncode == 0, result.stdout + result.stderr
    typed: Path = Path(__file__).parent / "fixtures" / "semantic" / "typed_orders.sql"
    (project / "sql" / "models" / "staging" / "stg_orders_typed.sql").write_text(
        typed.read_text(encoding="utf-8"), encoding="utf-8"
    )
    return project


@pytest.fixture
def cache_invalidation_project(tmp_path: Path) -> Path:
    project_dir: Path = tmp_path / "orders_project"
    prepare_compile_cache_invalidation_project(project_dir=project_dir)
    return project_dir


@pytest.fixture
def compile_reuse_project(tmp_path: Path) -> Path:
    project_dir: Path = tmp_path / "orders_project"
    prepare_compile_reuse_project(project_dir=project_dir)
    return project_dir


@pytest.fixture(scope="module")
def inspection_benchmark_project(tmp_path_factory: pytest.TempPathFactory) -> Path:
    assert_required_cgroup_memory_limit()
    project_dir: Path = tmp_path_factory.mktemp("inspection") / "semantic_inspection"
    prepare_inspection_benchmark_project(project_dir=project_dir)
    return project_dir


@pytest.fixture(scope="module")
def built_benchmark(tmp_path_factory: pytest.TempPathFactory) -> BuiltBenchmark:
    assert_required_cgroup_memory_limit()
    project_dir: Path = tmp_path_factory.mktemp("build") / "semantic_build"
    prepare_build_benchmark_project(project_dir=project_dir)
    build: InspectionCommandMeasurement = run_fresh_process_inspection_command(
        project_dir=project_dir,
        label="build-benchmark-build",
        sqb_args=("build",),
        expected_max_wall_seconds=300.0,
    )
    return BuiltBenchmark(project_dir=project_dir, build=build)
