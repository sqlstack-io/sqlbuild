"""A local build prints a running asset's `ctx.log` lines as they arrive, not when the asset ends."""

from pathlib import Path

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.build._test_types import (
    BlockedAssetBuild,
    LiveLogBuildTestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.main.build.helpers import (
    blocked_asset_project,
    build_until_line,
)


@pytest.mark.parametrize(
    "test_case",
    [
        LiveLogBuildTestCase(
            description="the first step prints while the asset still waits",
            expected_live_line="python    asset     stream_steps log  step 1 of 2",
            expected_later_line="python    asset     stream_steps log  step 2 of 2",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_a_running_asset_when_it_logs_then_the_local_build_prints_the_line_at_once(
    test_case: LiveLogBuildTestCase, tmp_path: Path
) -> None:
    project_dir, release = blocked_asset_project(tmp_path)

    build: BlockedAssetBuild = build_until_line(project_dir, release, test_case.expected_live_line)

    assert build.returncode == 0, build.output
    assert build.shown_before_release, build.output
    assert test_case.expected_later_line in build.output, build.output


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
