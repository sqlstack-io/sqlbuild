"""Tests for resolving the published baseline version."""

import pytest

from scripts.release_performance._helpers.versions import (
    choose_baseline,
    compatible_wheels,
    previous_version,
    release_versions,
)
from tests.unit.scripts.release_performance._helpers._test_types import (
    ChooseBaselineTestCase,
    CompatibleWheelsTestCase,
    PreviousVersionTestCase,
    ReleaseVersionsTestCase,
)
from tests.unit.scripts.release_performance._helpers.helpers import release_file

_RELEASE_FILES: tuple[str, ...] = (
    "sqlbuild-0.126.1-cp312-abi3-macosx_10_12_x86_64.whl",
    "sqlbuild-0.126.1-cp312-abi3-macosx_11_0_arm64.whl",
    "sqlbuild-0.126.1-cp312-abi3-manylinux_2_17_aarch64.manylinux2014_aarch64.whl",
    "sqlbuild-0.126.1-cp312-abi3-manylinux_2_17_x86_64.manylinux2014_x86_64.whl",
    "sqlbuild-0.126.1-cp312-abi3-win_amd64.whl",
    "sqlbuild-0.126.1.tar.gz",
    "sqlbuild-0.126.0-cp312-abi3-manylinux_2_17_x86_64.manylinux2014_x86_64.whl",
)


@pytest.mark.parametrize(
    "test_case",
    (
        PreviousVersionTestCase(
            description="the highest version below a release candidate",
            candidate="0.126.3",
            versions=("0.126.1", "0.126.2", "0.125.9", "0.126.2"),
            expected_version="0.126.2",
        ),
        PreviousVersionTestCase(
            description="an already published candidate compares with the release before it",
            candidate="0.126.2",
            versions=("0.126.1", "0.126.2"),
            expected_version="0.126.1",
        ),
        PreviousVersionTestCase(
            description="versions order numerically rather than as text",
            candidate="0.100.0",
            versions=("0.99.0", "0.9.9", "0.100.0"),
            expected_version="0.99.0",
        ),
        PreviousVersionTestCase(
            description="no earlier release",
            candidate="0.1.0",
            versions=("0.1.0",),
            expected_version=None,
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_known_versions_when_choosing_baseline_then_returns_previous_release(
    test_case: PreviousVersionTestCase,
) -> None:
    assert (
        previous_version(candidate=test_case.candidate, versions=test_case.versions)
        == test_case.expected_version
    )


@pytest.mark.parametrize(
    "test_case",
    (
        ReleaseVersionsTestCase(
            description="wheels and sdists of every release",
            files=tuple(release_file(filename=name) for name in _RELEASE_FILES),
            expected_versions=("0.126.0", "0.126.1"),
        ),
        ReleaseVersionsTestCase(
            description="a yanked release and a pre-release are not baselines",
            files=(
                {"filename": "sqlbuild-0.126.0.tar.gz", "yanked": False},
                {"filename": "sqlbuild-0.126.1.tar.gz", "yanked": "broken wheel"},
                {"filename": "sqlbuild-0.127.0rc1.tar.gz", "yanked": False},
            ),
            expected_versions=("0.126.0",),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_simple_index_when_listing_versions_then_returns_installable_releases(
    test_case: ReleaseVersionsTestCase,
) -> None:
    assert release_versions(index={"files": list(test_case.files)}) == test_case.expected_versions


@pytest.mark.parametrize(
    "test_case",
    (
        CompatibleWheelsTestCase(
            description="linux x86_64 runner",
            machine="x86_64",
            system="linux",
            expected_filenames=(
                "sqlbuild-0.126.1-cp312-abi3-manylinux_2_17_x86_64.manylinux2014_x86_64.whl",
            ),
        ),
        CompatibleWheelsTestCase(
            description="apple silicon",
            machine="arm64",
            system="darwin",
            expected_filenames=("sqlbuild-0.126.1-cp312-abi3-macosx_11_0_arm64.whl",),
        ),
        CompatibleWheelsTestCase(
            description="unsupported platform",
            machine="riscv64",
            system="linux",
            expected_filenames=(),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_release_files_when_selecting_wheel_urls_then_matches_runner_platform(
    test_case: CompatibleWheelsTestCase,
) -> None:
    index: dict[str, object] = {"files": [release_file(filename=name) for name in _RELEASE_FILES]}

    wheels: tuple[dict[str, object], ...] = compatible_wheels(
        index=index, version="0.126.1", machine=test_case.machine, system=test_case.system
    )

    assert tuple(entry["filename"] for entry in wheels) == test_case.expected_filenames


@pytest.mark.parametrize(
    "test_case",
    (
        ChooseBaselineTestCase(
            description="a main commit whose own release is not on PyPI uses the newest one that is",
            candidate="0.126.3",
            files=(
                release_file(filename="sqlbuild-0.126.1.tar.gz"),
                release_file(filename="sqlbuild-0.126.2.tar.gz"),
            ),
            same_version=True,
            expected_baseline="0.126.2",
        ),
        ChooseBaselineTestCase(
            description="a main commit uses its own release once PyPI serves it",
            candidate="0.126.2",
            files=(
                release_file(filename="sqlbuild-0.126.1.tar.gz"),
                release_file(filename="sqlbuild-0.126.2.tar.gz"),
            ),
            same_version=True,
            expected_baseline="0.126.2",
        ),
        ChooseBaselineTestCase(
            description="a release commit never compares with itself after it publishes",
            candidate="0.126.2",
            files=(
                release_file(filename="sqlbuild-0.126.1.tar.gz"),
                release_file(filename="sqlbuild-0.126.2.tar.gz"),
            ),
            same_version=False,
            expected_baseline="0.126.1",
        ),
        ChooseBaselineTestCase(
            description="a yanked release is skipped",
            candidate="0.126.3",
            files=(
                release_file(filename="sqlbuild-0.126.1.tar.gz"),
                {"filename": "sqlbuild-0.126.2.tar.gz", "yanked": "broken build", "url": ""},
            ),
            same_version=False,
            expected_baseline="0.126.1",
        ),
        ChooseBaselineTestCase(
            description="nothing precedes the first release",
            candidate="0.1.0",
            files=(release_file(filename="sqlbuild-0.1.0.tar.gz"),),
            same_version=False,
            expected_baseline=None,
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_published_releases_when_choosing_baseline_then_uses_one_pypi_serves(
    test_case: ChooseBaselineTestCase,
) -> None:
    baseline: str | None = choose_baseline(
        candidate=test_case.candidate,
        index={"files": list(test_case.files)},
        same_version=test_case.same_version,
    )

    assert baseline == test_case.expected_baseline


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
