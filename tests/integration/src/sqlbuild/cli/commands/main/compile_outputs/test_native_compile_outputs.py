"""Integration coverage for compile JSON, artifact writes, publication and reuse snapshots."""

import shutil
from pathlib import Path

import pytest

from tests.integration.src.sqlbuild.cli.commands.main.compile_outputs._test_types import (
    CompileOutputsSequenceTestCase,
    PublicationFailureTestCase,
)
from tests.integration.src.sqlbuild.cli.commands.main.compile_outputs.helpers import (
    OUTPUT_PROJECT_FILES,
    CompileOutcome,
    artifacts_except,
    block_artifact,
    comparable,
    compile_files,
    compile_outputs,
    edit_sequence_step,
    main_compile,
    neutral_error,
    prepare_output_compiles,
    reuse_compile,
    staging_directories,
    uncached_reference,
    write_files,
)

_SEQUENCE_STEPS: tuple[str, ...] = ("cold", "warm", "touch", "retouch", "edit", "delete")
_PUBLICATION_ERROR: str = (
    "[Errno 21] Is a directory: "
    "'<project>/target/.sqlbuild-staging-<id>/compiled/models/orders_north.sql' -> "
    "'<project>/target/compiled/models/orders_north.sql'"
)


@pytest.mark.parametrize(
    "test_case",
    [
        CompileOutputsSequenceTestCase(
            description="compile writes outputs and reuses unchanged compiles",
            expected_exit_codes=(0, 0, 0, 0, 0, 0),
            expected_reused=(False, True, False, True, False, False),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_cached_edit_sequence_when_compiling_then_each_step_matches_uncached_compile(
    test_case: CompileOutputsSequenceTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    project_dir: Path = tmp_path / "project"
    write_files(project_dir=project_dir, files=OUTPUT_PROJECT_FILES)
    prepare_output_compiles(monkeypatch=monkeypatch, reuse_disabled="0")
    reused: list[bool] = []
    exit_codes: list[int] = []

    for step in _SEQUENCE_STEPS:
        edit_sequence_step(project_dir=project_dir, step=step)
        replayed, cached = reuse_compile(project_dir=project_dir, capsys=capsys)
        reused.append(replayed)
        exit_codes.append(cached[0])
        reference: CompileOutcome = uncached_reference(
            project_dir=project_dir, reference_dir=tmp_path / f"uncached_{step}", capsys=capsys
        )
        assert comparable(cached) == comparable(reference)

    assert tuple(exit_codes) == test_case.expected_exit_codes
    assert tuple(reused) == test_case.expected_reused


@pytest.mark.parametrize(
    "test_case",
    [
        PublicationFailureTestCase(
            description="compile reports the blocked move and recovers",
            expected_error=_PUBLICATION_ERROR,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_artifact_path_blocked_when_publishing_staged_artifacts_then_compile_fails_cleanly(
    test_case: PublicationFailureTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    write_files(project_dir=tmp_path, files=OUTPUT_PROJECT_FILES)
    prepare_output_compiles(monkeypatch=monkeypatch, reuse_disabled="1")
    published: CompileOutcome = compile_outputs(
        project_dir=tmp_path, args=("--no-cache",), capsys=capsys
    )
    blocked: Path = block_artifact(project_dir=tmp_path)
    untouched: dict[str, bytes] = artifacts_except(
        files=published[2], name="sql/models/orders_north.sql"
    )

    with pytest.raises(IsADirectoryError) as raised:
        _ = main_compile(project_dir=tmp_path)
    remaining: dict[str, bytes] = compile_files(project_dir=tmp_path)
    kept: str = (blocked / "keep.txt").read_text(encoding="utf-8")
    shutil.rmtree(blocked)
    recovered: CompileOutcome = compile_outputs(
        project_dir=tmp_path, args=("--no-cache",), capsys=capsys
    )
    reference: CompileOutcome = uncached_reference(
        project_dir=tmp_path,
        reference_dir=tmp_path.parent / f"{tmp_path.name}_uncached",
        capsys=capsys,
    )

    assert neutral_error(error=raised.value, project_dir=tmp_path) == test_case.expected_error
    assert staging_directories(tmp_path / "target") == []
    assert {name: remaining[name] for name in untouched} == untouched
    assert kept == "kept\n"
    assert comparable(recovered) == comparable(reference)
