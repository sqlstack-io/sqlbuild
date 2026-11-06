"""Stable constants for comparing a release candidate with the previous published release."""

from pathlib import Path

from scripts.release_performance.models import BenchmarkCommand
from tests.e2e.src.sqlbuild.cli.commands.main.compile.helpers import (
    SHARED_DIAMOND_HUB,
    SHARED_DIAMOND_ROLLUP,
)

MAX_WALL_RATIO: float = 1.25
MAX_CPU_RATIO: float = 1.25
MAX_RSS_RATIO: float = 1.25
MAX_COMPILE_TIME_RATIO: float = 1.15
MAX_EDIT_COMPILE_TIME_RATIO: float = 1.35
MIN_WALL_REGRESSION_SECONDS: float = 0.5
MIN_CPU_REGRESSION_SECONDS: float = 0.5
MIN_RSS_REGRESSION_BYTES: int = 32 * 1024 * 1024

DEFAULT_RUNS: int = 5
DEFAULT_PYTHON: str = "3.12"
DEFAULT_INSPECTION_MODELS: int = 3_000
DEFAULT_BUILD_MODELS: int = 1_000
DEFAULT_DENSE_MODELS: int = 3_000
COMMAND_TIMEOUT_SECONDS: float = 900.0

PACKAGE_NAME: str = "sqlbuild"
PYPI_SIMPLE_URL: str = "https://pypi.org/simple/sqlbuild/"
PYPI_SIMPLE_JSON: str = "application/vnd.pypi.simple.v1+json"
PYPI_REQUEST_TIMEOUT_SECONDS: float = 60.0
BASELINE_PUBLICATION_WAIT_SECONDS: float = 1_800.0
BASELINE_PUBLICATION_POLL_SECONDS: float = 30.0
WHEEL_PLATFORM_FAMILIES: dict[str, str] = {"linux": "linux", "darwin": "macosx", "win32": "win"}
WHEEL_MACHINE_ALIASES: dict[str, str] = {"amd64": "x86_64", "arm64": "aarch64"}
UNIVERSAL_WHEEL_MACHINE: str = "universal2"
REPO_ROOT: Path = Path(__file__).resolve().parents[2]

BASELINE_LABEL: str = "baseline"
CANDIDATE_LABEL: str = "candidate"
INSPECTION_PROJECT: str = "inspection"
BUILD_PROJECT: str = "build"
DENSE_PROJECT: str = "dense"
EXCLUDED_ENVIRONMENT_KEYS: frozenset[str] = frozenset({"VIRTUAL_ENV", "GITHUB_STEP_SUMMARY"})
EXCLUDED_ENVIRONMENT_PREFIX: str = "DBT_"
TIME_FORMAT: str = "%e %M %U %S"
COMPILE_COMMAND: str = "compile"
STDERR_TAIL_CHARACTERS: int = 2_000
PRISTINE_DIRECTORY: str = "pristine"
TARGET_DIRECTORY: str = "target"
BASELINE_GENERATED_DIRECTORY: str = "baseline-generated"
BASELINE_SOURCE_DIRECTORY: str = "baseline-source"
RELEASE_TAG_PREFIX: str = "v"
BASELINE_GENERATOR_ENTRY: str = (
    "import sys; from pathlib import Path; "
    "from scripts.release_performance._helpers.benchmark import write_pristine_projects; "
    "write_pristine_projects("
    "root=Path(sys.argv[1]), inspection_models=int(sys.argv[2]), build_models=int(sys.argv[3]))"
)
DENSE_GENERATOR_ENTRY: str = (
    "import sys; from pathlib import Path; "
    "from scripts.cold_compile_performance._helpers.dense_project import "
    "write_dense_compile_project; "
    "write_dense_compile_project(project_dir=Path(sys.argv[1]), model_count=int(sys.argv[2]))"
)

BENCHMARK_COMMANDS: tuple[BenchmarkCommand, ...] = (
    BenchmarkCommand(
        name="compile (no cache)",
        project=INSPECTION_PROJECT,
        sqb_args=("compile", "--json", "--no-cache"),
        max_time_ratio=MAX_COMPILE_TIME_RATIO,
    ),
    BenchmarkCommand(
        name="compile (warm cache)",
        project=INSPECTION_PROJECT,
        sqb_args=("compile", "--json"),
        max_time_ratio=MAX_COMPILE_TIME_RATIO,
    ),
    BenchmarkCommand(
        name="plan --json",
        project=INSPECTION_PROJECT,
        sqb_args=("plan", "--json"),
    ),
    BenchmarkCommand(
        name="lineage hub downstream",
        project=INSPECTION_PROJECT,
        sqb_args=("lineage", SHARED_DIAMOND_HUB, "--direction", "downstream"),
    ),
    BenchmarkCommand(
        name="lineage rollup upstream",
        project=INSPECTION_PROJECT,
        sqb_args=("lineage", SHARED_DIAMOND_ROLLUP, "--direction", "upstream"),
    ),
    BenchmarkCommand(
        name="lineage column trace",
        project=INSPECTION_PROJECT,
        sqb_args=("lineage", f"{SHARED_DIAMOND_ROLLUP}.amount", "--direction", "upstream"),
    ),
    BenchmarkCommand(
        name="dag --json",
        project=INSPECTION_PROJECT,
        sqb_args=("dag", "--json"),
    ),
    BenchmarkCommand(
        name="scope --json",
        project=INSPECTION_PROJECT,
        sqb_args=(
            "scope",
            f"model:{SHARED_DIAMOND_ROLLUP}",
            "--json",
            "--globals",
            "all",
            "--include-nearby",
        ),
    ),
    BenchmarkCommand(
        name="compile (one-model edit)",
        project=INSPECTION_PROJECT,
        sqb_args=("compile", "--json"),
        edits_model=True,
        max_time_ratio=MAX_EDIT_COMPILE_TIME_RATIO,
    ),
    BenchmarkCommand(
        name="dense compile (warm cache)",
        project=DENSE_PROJECT,
        sqb_args=("compile", "--json"),
        max_time_ratio=MAX_COMPILE_TIME_RATIO,
    ),
    BenchmarkCommand(
        name="dense compile (one-model edit)",
        project=DENSE_PROJECT,
        sqb_args=("compile", "--json"),
        edits_model=True,
        max_time_ratio=MAX_EDIT_COMPILE_TIME_RATIO,
    ),
    BenchmarkCommand(
        name="dense compile (no cache)",
        project=DENSE_PROJECT,
        sqb_args=("compile", "--json", "--no-cache"),
        removes_target=True,
        max_time_ratio=MAX_COMPILE_TIME_RATIO,
    ),
    BenchmarkCommand(
        name="build (empty warehouse)",
        project=BUILD_PROJECT,
        sqb_args=("build",),
    ),
)

INSPECTION_WARMUPS: tuple[tuple[str, ...], ...] = (
    (COMPILE_COMMAND, "--json"),
    ("lineage", SHARED_DIAMOND_HUB, "--depth", "1"),
    ("scope", f"model:{SHARED_DIAMOND_ROLLUP}", "--json"),
)
BUILD_WARMUPS: tuple[tuple[str, ...], ...] = ((COMPILE_COMMAND, "--json"),)
DENSE_WARMUPS: tuple[tuple[str, ...], ...] = ((COMPILE_COMMAND, "--json"),)
