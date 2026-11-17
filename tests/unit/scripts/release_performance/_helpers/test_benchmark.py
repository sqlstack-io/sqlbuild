"""Unit tests for generating each version's release benchmark projects."""

from __future__ import annotations

from pathlib import Path

import pytest

from scripts.release_performance._helpers import benchmark, versions
from scripts.release_performance._helpers.benchmark import (
    write_baseline_dense_project,
    write_baseline_pristine_projects,
)
from scripts.release_performance._helpers.versions import release_source
from scripts.release_performance.exceptions import ReleasePerformanceError
from tests.unit.scripts.release_performance._helpers._test_types import (
    BaselineDenseGeneratorTestCase,
    BaselineGeneratorTestCase,
    HangingSetupPhaseTestCase,
    ReleaseSourceTestCase,
)
from tests.unit.scripts.release_performance._helpers.helpers import (
    BASELINE_ONLY_SYMBOL,
    BASELINE_SQLBUILD_DENSE_GENERATOR,
    BASELINE_SQLBUILD_GENERATOR,
    FAILING_DENSE_GENERATOR,
    FAILING_GENERATOR,
    MARKER_DENSE_GENERATOR,
    MARKER_GENERATOR,
    generate_with_hanging_generator,
    read_hanging_sqb_version,
    tagged_repository,
    write_baseline_dense_source,
    write_baseline_environment,
    write_baseline_source,
)


@pytest.mark.parametrize(
    "test_case",
    [
        BaselineGeneratorTestCase(
            description="the baseline source's own generator writes the baseline projects",
            generator=MARKER_GENERATOR,
            expected_projects=("build", "inspection"),
            expected_error_fragments=(),
        ),
        BaselineGeneratorTestCase(
            description="the generator imports the baseline environment's sqlbuild, not this one",
            generator=BASELINE_SQLBUILD_GENERATOR,
            expected_projects=("build", "inspection"),
            expected_error_fragments=(),
        ),
        BaselineGeneratorTestCase(
            description="a failing baseline generator is reported with its stderr",
            generator=FAILING_GENERATOR,
            expected_projects=(),
            expected_error_fragments=(
                "Baseline benchmark generation with the generator in",
                "baseline generator exploded",
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_baseline_source_when_generating_then_uses_its_own_generator(
    test_case: BaselineGeneratorTestCase, tmp_path: Path
) -> None:
    source: Path = write_baseline_source(root=tmp_path / "source", generator=test_case.generator)
    root: Path = tmp_path / "generated"
    error: str = ""

    try:
        _ = write_baseline_pristine_projects(
            source=source,
            python=write_baseline_environment(root=tmp_path / "venv"),
            root=root,
            inspection_models=30,
            build_models=10,
        )
    except ReleasePerformanceError as raised:
        error = str(raised)

    pristine: Path = root / "pristine"
    assert tuple(sorted(path.parent.name for path in pristine.glob("*/BASELINE_MARKER"))) == (
        test_case.expected_projects
    )
    assert all(fragment in error for fragment in test_case.expected_error_fragments), error


@pytest.mark.parametrize(
    "test_case",
    [
        BaselineDenseGeneratorTestCase(
            description="the baseline source's own dense generator writes the dense project",
            generator=MARKER_DENSE_GENERATOR,
            expected_markers=("40",),
            expected_error_fragments=(),
        ),
        BaselineDenseGeneratorTestCase(
            description="the dense generator imports the baseline environment's sqlbuild",
            generator=BASELINE_SQLBUILD_DENSE_GENERATOR,
            expected_markers=(BASELINE_ONLY_SYMBOL,),
            expected_error_fragments=(),
        ),
        BaselineDenseGeneratorTestCase(
            description="a failing baseline dense generator is reported with its stderr",
            generator=FAILING_DENSE_GENERATOR,
            expected_markers=(),
            expected_error_fragments=(
                "Baseline dense benchmark generation with the generator in",
                "baseline dense generator exploded",
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_baseline_source_when_generating_dense_project_then_uses_its_own_generator(
    test_case: BaselineDenseGeneratorTestCase, tmp_path: Path
) -> None:
    source: Path = write_baseline_dense_source(
        root=tmp_path / "source", generator=test_case.generator
    )
    pristine: Path = tmp_path / "generated" / "pristine"
    error: str = ""

    try:
        write_baseline_dense_project(
            source=source,
            python=write_baseline_environment(root=tmp_path / "venv"),
            pristine=pristine,
            models=40,
        )
    except ReleasePerformanceError as raised:
        error = str(raised)

    markers: tuple[str, ...] = tuple(
        marker.read_text(encoding="utf-8") for marker in pristine.glob("dense/BASELINE_MARKER")
    )
    assert markers == test_case.expected_markers
    assert all(fragment in error for fragment in test_case.expected_error_fragments), error


@pytest.mark.parametrize(
    "test_case",
    [
        ReleaseSourceTestCase(
            description="the release tag's tree is extracted",
            tag="v1.2.3",
            requested_version="1.2.3",
            expected_files=("release.txt",),
            expected_error_fragments=(),
        ),
        ReleaseSourceTestCase(
            description="a missing release tag explains how to provide the source",
            tag="v1.2.3",
            requested_version="1.2.2",
            expected_files=(),
            expected_error_fragments=("Release tag v1.2.2 is not available", "--baseline-source"),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_release_tag_when_extracting_source_then_returns_its_tree(
    test_case: ReleaseSourceTestCase, tmp_path: Path
) -> None:
    repository: Path = tagged_repository(root=tmp_path / "repo", tag=test_case.tag)
    destination: Path = tmp_path / "source"
    error: str = ""

    try:
        _ = release_source(
            version=test_case.requested_version, repo_dir=repository, destination=destination
        )
    except ReleasePerformanceError as raised:
        error = str(raised)

    assert tuple(sorted(path.name for path in destination.glob("*"))) == test_case.expected_files
    assert all(fragment in error for fragment in test_case.expected_error_fragments), error


@pytest.mark.parametrize(
    "test_case",
    (
        HangingSetupPhaseTestCase(
            description="a hanging baseline generator is stopped and named",
            run_phase=generate_with_hanging_generator,
            expected_error=lambda _root: (
                "Baseline benchmark generation did not finish within 2s and was stopped"
            ),
        ),
        HangingSetupPhaseTestCase(
            description="a hanging sqb --version is stopped and named",
            run_phase=read_hanging_sqb_version,
            expected_error=lambda root: (
                f"Reading the version of {root / 'sqb'} did not finish within 2s and was stopped"
            ),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_a_hanging_setup_command_when_comparing_then_fails_naming_its_phase(
    test_case: HangingSetupPhaseTestCase, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    monkeypatch.setattr(benchmark, "SETUP_COMMAND_TIMEOUT_SECONDS", 2.0)
    monkeypatch.setattr(versions, "SETUP_COMMAND_TIMEOUT_SECONDS", 2.0)

    with pytest.raises(ReleasePerformanceError) as raised:
        _ = test_case.run_phase(tmp_path)

    assert str(raised.value) == test_case.expected_error(tmp_path)


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
