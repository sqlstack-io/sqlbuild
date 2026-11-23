"""E2E coverage for early build failures in a real terminal."""

from __future__ import annotations

import subprocess
from pathlib import Path

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.build._test_types import (
    TerminalBuildFailureE2ETestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import (
    SPINNER_GLYPHS,
    prepare_waffle_shop,
    render_terminal_screen,
    run_sqb_with_pty,
)


@pytest.mark.parametrize(
    "test_case",
    [
        TerminalBuildFailureE2ETestCase(
            description="selected model with unbuilt upstreams fails during planning",
            command=("build", "--select", "daily_revenue"),
            model_replacements=(),
            expected_screen_fragments=(
                "Inspected warehouse state.",
                "error[S301]: 2 upstream models this build reads don't exist in the warehouse",
            ),
        ),
        TerminalBuildFailureE2ETestCase(
            description="compile error fails before connecting",
            command=("build",),
            model_replacements=(
                (
                    "sql/models/marts/daily_revenue.sql",
                    '__ref("stg_payments")',
                    '__ref("missing_payments")',
                ),
            ),
            expected_screen_fragments=("error[", "missing_payments"),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_early_build_failure_when_running_in_terminal_then_exits_with_error_only(
    tmp_path: Path, test_case: TerminalBuildFailureE2ETestCase
) -> None:
    project_dir: Path = prepare_waffle_shop(tmp_path)
    for relative_path, old, new in test_case.model_replacements:
        model_path: Path = project_dir / relative_path
        model_path.write_text(
            model_path.read_text(encoding="utf-8").replace(old, new), encoding="utf-8"
        )

    result: subprocess.CompletedProcess[str] = run_sqb_with_pty(
        command=test_case.command, project_dir=project_dir, columns=test_case.columns
    )
    screen: tuple[str, ...] = render_terminal_screen(
        output=result.stdout, columns=test_case.columns
    )
    rendered: str = "\n".join(screen)

    assert result.returncode == 1, rendered
    assert "Traceback" not in result.stdout, rendered
    assert "Exception ignored" not in result.stdout, rendered
    assert not any(glyph in rendered for glyph in SPINNER_GLYPHS), rendered
    assert all(fragment in rendered for fragment in test_case.expected_screen_fragments), rendered


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
