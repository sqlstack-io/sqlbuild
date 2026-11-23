from __future__ import annotations

import contextlib
import sys
from pathlib import Path

import pytest

from sqlbuild._native import lineage_cache_fingerprint
from tests.unit.src.sqlbuild.cli.commands._helpers.lineage._test_types import (
    FingerprintedFilesTestCase,
    InterruptedListingFingerprintTestCase,
    LineageFingerprintAvailabilityTestCase,
    LineageFingerprintEnvironmentTestCase,
    UncacheableFingerprintTestCase,
)
from tests.unit.src.sqlbuild.cli.commands._helpers.lineage.helpers import (
    expected_fingerprint,
    fingerprint_prefix,
    interrupt_listing,
)

_LAYOUT_FILES: dict[str, str] = {
    "sqlbuild_project.toml": 'name = "orders"\nadapter = "duckdb"\nschema = "${ENV:ORDERS_SCHEMA}"\n',
    "sql/models/a/b.sql": "SELECT 1 AS order_id\n",
    "sql/models/a-c.sql": "SELECT '${ENV:  ORDERS_REGION}' AS region\n",
    "sql/models/a.sql": "SELECT 2 AS order_id\n",
    "sql/models/Z.SQL": "SELECT 3 AS order_id\n",
    "sql/models/é.sql": "SELECT 'café' AS label\n",
    "sql/seeds/customers.csv": "customer_id\n1\n",
    "sql/sources/raw.YML": "sources: []\n",
    "sql/macros/python/helpers.py": "def total():\n    return 1\n",
    ".hidden/notes.yaml": "a: 1\n",
    ".gitignore": "target/\n",
    "nested/.sqlbuildignore": "*.tmp\n",
    ".sql": "SELECT 'not a suffix'\n",
    "README.md": "${CTX:ignored}\n",
    "noext": "ENV:\n",
    "target/ignored.sql": "ENV:\n",
    ".venv/ignored.py": "ENV:\n",
    "sql/models/__pycache__/ignored.py": "ENV:\n",
    "nested/target/kept.sql": "SELECT 4 AS order_id\n",
}
_LAYOUT_LINKS: dict[str, str] = {
    "sql/models/linked.sql": "sql/models/a.sql",
    "linked_dir": "models",
    "sql/models/broken.sql": "sql/models/missing.sql",
}
_LAYOUT_UNLINKED_HASHED_FILES: tuple[str, ...] = (
    ".gitignore",
    ".hidden/notes.yaml",
    "sql/macros/python/helpers.py",
    "sql/models/Z.SQL",
    "sql/models/a/b.sql",
    "sql/models/a-c.sql",
    "sql/models/a.sql",
    "sql/models/é.sql",
    "nested/.sqlbuildignore",
    "nested/target/kept.sql",
    "sql/seeds/customers.csv",
    "sql/sources/raw.YML",
    "sqlbuild_project.toml",
)
_LAYOUT_HASHED_FILES: tuple[str, ...] = (*_LAYOUT_UNLINKED_HASHED_FILES, "sql/models/linked.sql")
_LAYOUT_ENVIRONMENT_NAMES: tuple[str, ...] = ("ORDERS_REGION", "ORDERS_SCHEMA")


@pytest.mark.parametrize(
    "test_case",
    (
        LineageFingerprintEnvironmentTestCase(
            description="referenced environment value participates in cache identity",
            config=(
                'name = "orders"\nadapter = "duckdb"\n'
                '[targets.dev]\nschema = "${if(eq( ENV:ORDERS_SCHEMA, '
                "'orders_dev'), 'dev', 'test')}\"\n"
            ),
            environment_name="ORDERS_SCHEMA",
            first_value="orders_dev",
            second_value="orders_test",
            expected_equal=False,
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_referenced_environment_change_when_fingerprinting_then_invalidates_cache_identity(
    test_case: LineageFingerprintEnvironmentTestCase,
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text(test_case.config, encoding="utf-8")
    monkeypatch.setenv(test_case.environment_name, test_case.first_value)
    first: str | None = lineage_cache_fingerprint(project_dir=tmp_path, cli_vars=None)
    monkeypatch.setenv(test_case.environment_name, test_case.second_value)

    second: str | None = lineage_cache_fingerprint(project_dir=tmp_path, cli_vars=None)

    assert (first == second) is test_case.expected_equal


@pytest.mark.parametrize(
    "test_case",
    (
        LineageFingerprintAvailabilityTestCase(
            description="dynamic invocation context disables structural cache",
            relative_path="sqlbuild_project.toml",
            config=(
                'name = "orders"\nadapter = "duckdb"\n'
                '[targets.dev]\nschema = "orders_${CTX:run_id}"\n'
            ),
            expected_available=False,
        ),
        LineageFingerprintAvailabilityTestCase(
            description="unicode environment name disables structural cache",
            relative_path="sqlbuild_project.toml",
            config=(
                'name = "orders"\nadapter = "duckdb"\n[targets.dev]\nschema = "${ENV:ORDERS_ÉTÉ}"\n'
            ),
            expected_available=False,
        ),
        LineageFingerprintAvailabilityTestCase(
            description="SQL output context preserves structural cache availability",
            relative_path="sql/models/orders.sql",
            config=(
                "MODEL (description 'Test model orders.', materialized view);\n\n"
                "SELECT '${CTX:run_id}' AS invocation_id, 1 AS order_id\n"
            ),
            expected_available=True,
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_dynamic_invocation_context_when_fingerprinting_then_disables_cache(
    test_case: LineageFingerprintAvailabilityTestCase,
    tmp_path: Path,
) -> None:
    authored_path: Path = tmp_path / test_case.relative_path
    authored_path.parent.mkdir(parents=True, exist_ok=True)
    authored_path.write_text(test_case.config, encoding="utf-8")

    observed: str | None = lineage_cache_fingerprint(project_dir=tmp_path, cli_vars=None)

    assert (observed is not None) is test_case.expected_available


@pytest.mark.parametrize(
    "test_case",
    (
        FingerprintedFilesTestCase(
            description="sorting, suffixes, exclusions, links and environment values",
            files=_LAYOUT_FILES,
            links=_LAYOUT_LINKS,
            unreadable_directories=(),
            environment={"ORDERS_SCHEMA": "orders_dev"},
            cli_vars={"region": "north", "limits": [1, 2.5, None], "label": "café"},
            expected_hashed_files=_LAYOUT_HASHED_FILES,
            expected_environment_names=_LAYOUT_ENVIRONMENT_NAMES,
        ),
        FingerprintedFilesTestCase(
            description="chmod permissions follow the platform directory semantics",
            files={**_LAYOUT_FILES, "sql/models/locked/hidden.sql": "SELECT 5 AS order_id\n"},
            links={},
            unreadable_directories=("sql/models/locked",),
            environment={"ORDERS_SCHEMA": "orders_dev"},
            cli_vars={},
            expected_hashed_files=_LAYOUT_UNLINKED_HASHED_FILES
            + (("sql/models/locked/hidden.sql",) if sys.platform == "win32" else ()),
            expected_environment_names=_LAYOUT_ENVIRONMENT_NAMES,
        ),
        FingerprintedFilesTestCase(
            description="undecodable names that are never hashed",
            files={
                **_LAYOUT_FILES,
                "sql/models/notes_\udcff.md": "ENV:\n",
                "sql/models/\udcfe_dir/readme.txt": "ENV:\n",
            },
            links={},
            unreadable_directories=(),
            environment={"ORDERS_SCHEMA": "orders_dev"},
            cli_vars={},
            expected_hashed_files=_LAYOUT_UNLINKED_HASHED_FILES,
            expected_environment_names=_LAYOUT_ENVIRONMENT_NAMES,
        ),
        FingerprintedFilesTestCase(
            description="an undecodable hashed file inside an excluded directory",
            files={**_LAYOUT_FILES, "target/orders_\udcff.sql": "SELECT 8 AS order_id\n"},
            links={},
            unreadable_directories=(),
            environment={"ORDERS_SCHEMA": "orders_dev"},
            cli_vars={},
            expected_hashed_files=_LAYOUT_UNLINKED_HASHED_FILES,
            expected_environment_names=_LAYOUT_ENVIRONMENT_NAMES,
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_authored_files_when_fingerprinting_then_hashes_exactly_the_expected_inputs(
    test_case: FingerprintedFilesTestCase,
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    for relative_path, contents in test_case.files.items():
        path: Path = tmp_path / relative_path
        path.parent.mkdir(parents=True, exist_ok=True)
        _ = path.write_text(contents, encoding="utf-8")
    for relative_path, target in test_case.links.items():
        (tmp_path / relative_path).symlink_to(tmp_path / target)
    for relative_path in test_case.unreadable_directories:
        (tmp_path / relative_path).chmod(0)
    for name, value in test_case.environment.items():
        monkeypatch.setenv(name, value)
    monkeypatch.delenv("ORDERS_REGION", raising=False)

    observed: str | None = lineage_cache_fingerprint(
        project_dir=tmp_path, cli_vars=test_case.cli_vars
    )

    for relative_path in test_case.unreadable_directories:
        (tmp_path / relative_path).chmod(0o755)
    assert observed == expected_fingerprint(
        project_dir=tmp_path,
        prefix=fingerprint_prefix(cli_vars=test_case.cli_vars),
        hashed_files=test_case.expected_hashed_files,
        environment_names=test_case.expected_environment_names,
    )


@pytest.mark.parametrize(
    "test_case",
    (
        UncacheableFingerprintTestCase(
            description="an environment marker without a name",
            files={**_LAYOUT_FILES, "sql/models/bad.sql": "SELECT 'ENV: ' AS broken\n"},
            expected_fingerprint=None,
        ),
        UncacheableFingerprintTestCase(
            description="an environment name followed by a non-ASCII byte",
            files={"sql/models/bad.sql": "SELECT '${ENV:ORDERSÉ}' AS broken\n"},
            expected_fingerprint=None,
        ),
        UncacheableFingerprintTestCase(
            description="overlapping markers inside one matched name",
            files={"sql/models/bad.sql": "SELECT 'ENV: ENV:X' AS broken\n"},
            expected_fingerprint=None,
        ),
        UncacheableFingerprintTestCase(
            description="dynamic context in Python source",
            files={
                "sql/macros/python/run.py": "RUN = 'CTX:run_id'\n",
                "sql/models/a.sql": "SELECT 1\n",
            },
            expected_fingerprint=None,
        ),
        UncacheableFingerprintTestCase(
            description="an undecodable hashed file name",
            files={**_LAYOUT_FILES, "sql/models/orders_\udcff.sql": "SELECT 6 AS order_id\n"},
            expected_fingerprint=None,
        ),
        UncacheableFingerprintTestCase(
            description="a hashed file below an undecodable directory",
            files={**_LAYOUT_FILES, "sql/models/\udcfe_dir/orders.sql": "SELECT 7 AS order_id\n"},
            expected_fingerprint=None,
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_unhashable_inputs_when_fingerprinting_then_the_cache_is_disabled(
    test_case: UncacheableFingerprintTestCase, tmp_path: Path
) -> None:
    for relative_path, contents in test_case.files.items():
        path: Path = tmp_path / relative_path
        path.parent.mkdir(parents=True, exist_ok=True)
        _ = path.write_text(contents, encoding="utf-8")

    observed: str | None = lineage_cache_fingerprint(project_dir=tmp_path, cli_vars=None)

    assert observed is test_case.expected_fingerprint


@pytest.mark.parametrize(
    "test_case",
    (
        InterruptedListingFingerprintTestCase(
            description="a hashed directory whose listing fails mid-way",
            files={"sqlbuild_project.toml": 'name = "orders"\n', "sql/models/a.sql": "SELECT 1\n"},
            interrupted_directory="models",
            expected_uncacheable_before=(3, 13),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_interrupted_listing_when_python_globs_then_native_policy_matches_rglob(
    test_case: InterruptedListingFingerprintTestCase,
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    for relative_path, contents in test_case.files.items():
        path: Path = tmp_path / relative_path
        path.parent.mkdir(parents=True, exist_ok=True)
        _ = path.write_text(contents, encoding="utf-8")
    interrupt_listing(monkeypatch=monkeypatch, directory=tmp_path / test_case.interrupted_directory)
    listed: list[Path] = []

    with contextlib.suppress(OSError):
        listed = list(tmp_path.rglob("*"))

    assert (not listed) is (sys.version_info < test_case.expected_uncacheable_before)
