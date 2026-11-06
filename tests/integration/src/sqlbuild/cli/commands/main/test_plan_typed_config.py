"""Planning retains native config trees and leaves unread Python containers unopened."""

from __future__ import annotations

from pathlib import Path

import pytest

from tests.integration.src.sqlbuild.cli.commands.main._test_types import (
    TypedConfigReadsTestCase,
)
from tests.integration.src.sqlbuild.cli.commands.main.helpers import typed_config_reads


@pytest.mark.parametrize(
    "test_case",
    [
        TypedConfigReadsTestCase(
            description="row-diff settings no planner reader reads",
            unread_keys=(
                ', row_diff_exclude_columns ["loaded_at"], row_diff_sample_seed 7, '
                "row_diff_tolerances (default 1)"
            ),
            expected_same_reads=True,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_config_keys_planning_never_reads_when_planning_then_they_are_not_converted(
    test_case: TypedConfigReadsTestCase, tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    plain: int = typed_config_reads(project_dir=tmp_path / "plain", extra_header="")
    extended: int = typed_config_reads(
        project_dir=tmp_path / "extended", extra_header=test_case.unread_keys
    )
    _ = capsys.readouterr()

    assert (plain, extended == plain) == (0, test_case.expected_same_reads)
