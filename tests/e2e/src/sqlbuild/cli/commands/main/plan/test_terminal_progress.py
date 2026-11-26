"""E2E coverage for plan progress and migration output in a real terminal."""

from __future__ import annotations

import subprocess
from pathlib import Path

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.model_migrations.helpers import (
    build,
    load_raw_orders,
    write_orders_project,
)
from tests.e2e.src.sqlbuild.cli.commands.main.plan._test_types import (
    TerminalPlanProgressE2ETestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import (
    SPINNER_GLYPHS,
    assert_fragments_in_order,
    render_terminal_screen,
    run_sqb_with_pty,
)


@pytest.mark.parametrize(
    "test_case",
    [
        TerminalPlanProgressE2ETestCase(
            description="renamed model plan leaves only persistent lines on screen",
            runs=3,
            columns=100,
            expected_screen_fragments=(
                "\u2713 Project compiled",
                "\u2713 Rules evaluated  (",
                "s; built-in ",
                "Connecting to duckdb...",
                "✓ Warehouse connected  duckdb",
                "\u2713 Plan generated",
                "Plan ready  1 selected",
                "Migrations (1)",
                "└── stg_customer_orders  migrate  main.stg_orders -> main.stg_customer_orders",
                "├── compatibility  compatible",
                "Models (1)",
            ),
            expected_raw_fragments=(
                "\x1b[1mMigrations\x1b[0m \x1b[2m(1)\x1b[0m",
                "\x1b[34m\x1b[1mmigrate\x1b[0m",
                "\x1b[2mmain.stg_orders ->\x1b[0m main.stg_customer_orders",
                "\x1b[2mcompatibility\x1b[0m  \x1b[32mcompatible\x1b[0m",
                "\x1b[1mModels\x1b[0m \x1b[2m(1)\x1b[0m",
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_renamed_model_when_planning_in_terminal_then_no_transient_line_survives(
    tmp_path: Path, test_case: TerminalPlanProgressE2ETestCase
) -> None:
    project_dir: Path = write_orders_project(tmp_path=tmp_path, name="stg_orders", migrate_from="")
    load_raw_orders(project_dir=project_dir, last_day=3)
    initial: subprocess.CompletedProcess[str] = build(project_dir=project_dir)
    _ = write_orders_project(
        tmp_path=tmp_path, name="stg_customer_orders", migrate_from="stg_orders"
    )

    results: list[subprocess.CompletedProcess[str]] = [
        run_sqb_with_pty(command=("plan",), project_dir=project_dir, columns=test_case.columns)
        for _ in range(test_case.runs)
    ]

    assert initial.returncode == 0, initial.stdout + initial.stderr
    for result in results:
        screen: tuple[str, ...] = render_terminal_screen(
            output=result.stdout, columns=test_case.columns
        )
        rendered: str = "\n".join(screen)
        assert result.returncode == 0, rendered
        assert not any(glyph in rendered for glyph in SPINNER_GLYPHS), rendered
        assert any(line.startswith("✓ Warehouse connected") for line in screen), rendered
        assert_fragments_in_order(rendered, test_case.expected_screen_fragments)
        assert all(fragment in result.stdout for fragment in test_case.expected_raw_fragments)


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
