from pathlib import Path

import pytest

from scripts.native_layering._helpers.versions import get_native_version_errors
from tests.unit.scripts.native_layering._helpers._test_types import (
    NativeVersionTestCase,
    RepositoryLayeringTestCase,
)
from tests.unit.scripts.native_layering._helpers.helpers import write_versions


@pytest.mark.parametrize(
    "test_case",
    [
        NativeVersionTestCase(
            description="release crates agree with the release while the rest keep a fixed version",
            workspace_version="0.0.0",
            crate_version="1.2.0",
            pyproject_version="1.2.0",
            release_version="1.2.0",
            cli_version="1.2.0",
            expected_errors=(),
            lock_versions={"example-core": "0.0.0"},
            version_readers=("sqlbuild-cli", "sqlbuild-python"),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_matching_versions_when_checking_then_returns_no_errors(
    tmp_path: Path, test_case: NativeVersionTestCase
) -> None:
    write_versions(tmp_path, test_case)

    assert get_native_version_errors(tmp_path) == test_case.expected_errors


@pytest.mark.parametrize(
    "test_case",
    [
        NativeVersionTestCase(
            description="release crate versions lag a release",
            workspace_version="0.0.0",
            crate_version="1.1.9",
            pyproject_version="1.2.0",
            release_version="1.2.0",
            cli_version="1.2.0",
            expected_errors=(
                "Release versions differ: crates/sqlbuild-cli/Cargo.toml version is 1.1.9, "
                "crates/sqlbuild-python/Cargo.toml version is 1.1.9, "
                "pyproject.toml [project] version is 1.2.0, "
                ".release-please-manifest.json version is 1.2.0, "
                "pyproject.toml sqlbuild-cli dependency is 1.2.0.",
            ),
        ),
        NativeVersionTestCase(
            description="native executable pin lags a release",
            workspace_version="0.0.0",
            crate_version="1.2.0",
            pyproject_version="1.2.0",
            release_version="1.2.0",
            cli_version="1.1.9",
            expected_errors=(
                "Release versions differ: crates/sqlbuild-cli/Cargo.toml version is 1.2.0, "
                "crates/sqlbuild-python/Cargo.toml version is 1.2.0, "
                "pyproject.toml [project] version is 1.2.0, "
                ".release-please-manifest.json version is 1.2.0, "
                "pyproject.toml sqlbuild-cli dependency is 1.1.9.",
            ),
        ),
        NativeVersionTestCase(
            description="lockfile kept the previous release crate versions after a release bump",
            workspace_version="0.0.0",
            crate_version="1.2.0",
            pyproject_version="1.2.0",
            release_version="1.2.0",
            cli_version="1.2.0",
            lock_versions={"sqlbuild-cli": "1.1.9", "sqlbuild-python": "1.1.9"},
            expected_errors=(
                "Cargo.lock workspace crate versions are stale: sqlbuild-cli is 1.1.9, not 1.2.0; "
                "sqlbuild-python is 1.1.9, not 1.2.0. Run `cargo update --workspace`.",
            ),
        ),
        NativeVersionTestCase(
            description="uv lockfile kept the previous project version after a release bump",
            workspace_version="0.0.0",
            crate_version="1.2.0",
            pyproject_version="1.2.0",
            release_version="1.2.0",
            cli_version="1.2.0",
            python_lock_version="1.1.9",
            expected_errors=(
                "uv.lock locks example at 1.1.9, not pyproject.toml 1.2.0. Run `uv lock`.",
            ),
        ),
        NativeVersionTestCase(
            description="a fixed-version crate lags its workspace version in the lockfile",
            workspace_version="0.0.0",
            crate_version="1.2.0",
            pyproject_version="1.2.0",
            release_version="1.2.0",
            cli_version="1.2.0",
            lock_versions={"example-core": "1.1.9"},
            expected_errors=(
                "Cargo.lock workspace crate versions are stale: example-core is 1.1.9, not 0.0.0. "
                "Run `cargo update --workspace`.",
            ),
        ),
        NativeVersionTestCase(
            description="a fixed-version crate reads its package version",
            workspace_version="0.0.0",
            crate_version="1.2.0",
            pyproject_version="1.2.0",
            release_version="1.2.0",
            cli_version="1.2.0",
            lock_versions={"example-core": "0.0.0"},
            version_readers=("example-core", "sqlbuild-cli"),
            expected_errors=(
                "crates/example-core/src/lib.rs reads CARGO_PKG_VERSION, which is the fixed "
                "workspace version outside sqlbuild-cli and sqlbuild-python; take the release "
                "version from those crates.",
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_diverged_versions_when_checking_then_reports_them(
    tmp_path: Path, test_case: NativeVersionTestCase
) -> None:
    write_versions(tmp_path, test_case)

    assert get_native_version_errors(tmp_path) == test_case.expected_errors


@pytest.mark.parametrize(
    "test_case",
    [
        RepositoryLayeringTestCase(
            description="repository release versions agree",
            expected_errors=(),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_repository_when_checking_versions_then_they_agree(
    test_case: RepositoryLayeringTestCase,
) -> None:
    repository: Path = Path(__file__).resolve().parents[5]

    assert get_native_version_errors(repository) == test_case.expected_errors


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
