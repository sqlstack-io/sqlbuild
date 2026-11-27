"""Progress messages of `sqb rename` and `sqb mv` against a real offline compile."""

from __future__ import annotations

from pathlib import Path

import pytest

from sqlbuild._native import CliCommand, RefactorCommandRequest, run_refactor_command
from tests.integration.src.sqlbuild.cli.commands._helpers.refactor._test_types import (
    BareTargetRefactorTestCase,
    RefactorStatusMessagesTestCase,
)

_PROJECT_FILES: dict[str, str] = {
    "sqlbuild_project.toml": (
        'name = "orders_project"\nadapter = "duckdb"\ndefault_target = "dev"\n\n'
        '[connection]\ndatabase = "orders.duckdb"\n\n'
        '[targets.dev]\nschema = "analytics"\n'
    ),
    "sql/seeds/orders.csv": "order_id,amount\n1,25\n2,40\n",
    "sql/seeds/orders.yml": (
        "seeds:\n"
        "  - name: orders\n    description: Test seed orders.\n"
        "    columns:\n"
        "      - name: order_id\n        type: INTEGER\n"
        "      - name: amount\n        type: INTEGER\n"
    ),
    "sql/models/staging/stg_orders.sql": (
        "MODEL (description 'Test model.',\n  materialized view,\n);\n\n"
        'SELECT\n  order_id,\n  amount\nFROM __seed("orders")\n'
    ),
    "sql/models/marts/fact_orders.sql": (
        "MODEL (description 'Test model.',\n  materialized table,\n);\n\n"
        'SELECT\n  order_id,\n  amount\nFROM __ref("stg_orders")\n'
    ),
}


@pytest.mark.parametrize(
    "test_case",
    [
        RefactorStatusMessagesTestCase(
            description="moving a model nothing else names edits one file",
            command=CliCommand.MV,
            target="model:fact_orders",
            new_name=None,
            destination="sql/models/finance/fact_orders.sql",
            expected_lines=(
                "✓ Edits to 1 file planned",
                "Writing 1 file...",
                "✓ 1 file written",
                "Compiled: ok, 1 file changed",
            ),
        ),
        RefactorStatusMessagesTestCase(
            description="renaming a column nothing downstream reads edits one file",
            command=CliCommand.RENAME,
            target="column:fact_orders.amount",
            new_name="order_amount",
            destination=None,
            expected_lines=(
                "✓ Edits to 1 file planned",
                "Writing 1 file...",
                "✓ 1 file written",
                "Compiled: ok, 1 file changed",
            ),
        ),
        RefactorStatusMessagesTestCase(
            description="renaming a model another model reads edits two files",
            command=CliCommand.RENAME,
            target="model:stg_orders",
            new_name="stg_order_lines",
            destination=None,
            expected_lines=(
                "✓ Edits to 2 files planned",
                "Writing 2 files...",
                "✓ 2 files written",
                "Compiled: ok, 2 files changed",
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_refactor_when_running_then_progress_messages_agree_in_number(
    test_case: RefactorStatusMessagesTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    relative_path: str
    content: str
    for relative_path, content in _PROJECT_FILES.items():
        path: Path = tmp_path / relative_path
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(content)

    exit_code: int = run_refactor_command(
        request=RefactorCommandRequest(
            command=test_case.command,
            target=test_case.target,
            new_name=test_case.new_name,
            destination=test_case.destination,
            project_dir=tmp_path,
            no_color=True,
        )
    )

    lines: list[str] = capsys.readouterr().out.splitlines()
    assert exit_code == 0, lines
    expected: str
    for expected in test_case.expected_lines:
        assert expected in lines, "\n".join(lines)


@pytest.mark.parametrize(
    "test_case",
    [
        BareTargetRefactorTestCase(
            description="bare model.column renames the column downstream",
            command=CliCommand.RENAME,
            target="stg_orders.amount",
            new_name="revenue",
            destination=None,
            expected_path="sql/models/marts/fact_orders.sql",
            expected_fragment="revenue AS amount",
            expected_missing_paths=(),
        ),
        BareTargetRefactorTestCase(
            description="bare model name moves the model file",
            command=CliCommand.MV,
            target="fact_orders",
            new_name=None,
            destination="sql/models/finance/",
            expected_path="sql/models/finance/fact_orders.sql",
            expected_fragment='FROM __ref("stg_orders")',
            expected_missing_paths=("sql/models/marts/fact_orders.sql",),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_bare_target_when_running_then_applies_refactor(
    test_case: BareTargetRefactorTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    relative_path: str
    content: str
    for relative_path, content in _PROJECT_FILES.items():
        path: Path = tmp_path / relative_path
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(content)

    exit_code: int = run_refactor_command(
        request=RefactorCommandRequest(
            command=test_case.command,
            target=test_case.target,
            new_name=test_case.new_name,
            destination=test_case.destination,
            project_dir=tmp_path,
            no_color=True,
        )
    )

    output: str = capsys.readouterr().out
    assert exit_code == 0, output
    assert test_case.expected_fragment in (tmp_path / test_case.expected_path).read_text()
    missing_path: str
    for missing_path in test_case.expected_missing_paths:
        assert not (tmp_path / missing_path).exists()


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
