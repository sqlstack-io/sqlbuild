"""E2E coverage that Snowflake backslash escapes hide nothing from compile-time scanners."""

from __future__ import annotations

import subprocess
from pathlib import Path

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.scenario._test_types import (
    EscapedQuoteScenarioTestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.main.scenario.helpers import (
    SNOWFLAKE_ORDER_IDS_FIXTURE_SQL,
    build_offline_snowflake_project_toml,
    write_snowflake_order_ids_snapshot,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import prepare_inline_project, run_sqb

_NOTE_SQL: str = '\'it\\\'s __ref("ghost") @py.ghost() @sql.ghost(1) @const("ghost")\''
_EXPECTED_ROW_SQL: str = (
    "SELECT 1 AS order_id, 'O\\'Brien' AS customer_name, 'EU' AS region, "
    "'2025-01-01' AS start_date, 'web' AS channel, 'A\\'B' AS shouted, 'tag' AS tag, "
    f"'C:\\\\ducks\\\\' AS folder, {_NOTE_SQL} AS note"
)
_PROJECT_FILES: dict[str, str] = {
    "sqlbuild_project.toml": build_offline_snowflake_project_toml() + '\n[vars]\nregion = "EU"\n',
    "sources/raw.yml": (
        "sources:\n  - name: raw_orders\n    description: Raw orders.\n"
        "    schema: main\n    table: raw_orders\n    columns:\n"
        '      - {name: order_id, type: "NUMBER(38,0)"}\n'
    ),
    "models/_constants/start_date.sql": 'CONSTANT (name start_date, value "2025-01-01");\n',
    "models/_enums/channel.sql": 'ENUM (name channel, members (WEB "web", APP "app"));\n',
    "models/_macros/sql/shout.sql": (
        'MACRO (description "Upper case.", arguments (v VARCHAR), returns VARCHAR);\n\nUPPER(@v)\n'
    ),
    "models/_macros/python/tag.py": (
        '"""Order tag macros."""\n\n\n'
        "def tag() -> str:\n"
        '    """Return the order tag literal."""\n'
        "    return \"'tag'\"\n"
    ),
    "models/order_notes.sql": (
        'MODEL (description "Order notes holding backslash escapes.");\n'
        "SELECT\n"
        "  o.order_id,\n"
        "  'O\\'Brien' AS customer_name,\n"
        "  '@@region' AS region,\n"
        '  @const("start_date") AS start_date,\n'
        '  @enum("channel").WEB AS channel,\n'
        "  @sql.shout('a\\'b') AS shouted,\n"
        "  @py.tag() AS tag,\n"
        "  'C:\\\\ducks\\\\' AS folder, // it's a path\n"
        f"  {_NOTE_SQL} AS note\n"
        'FROM __source("raw_orders") AS o\n'
    ),
    "tests/unit/test_order_notes.sql": (
        'TEST (name "order_notes__keeps_escaped_quotes");\n\n'
        "WITH __source__raw_orders AS (SELECT 1 AS order_id, 'it\\'s' AS unused),\n\n"
        f"__expected__order_notes AS ({_EXPECTED_ROW_SQL})\n"
    ),
    "tests/scenarios/order_notes__escapes.sql": (
        "SCENARIO (description 'Escaped quotes replay locally.');\n"
        "WITH\n"
        f"__source__raw_orders AS ({SNOWFLAKE_ORDER_IDS_FIXTURE_SQL}),\n"
        f"__expected__order_notes AS ({_EXPECTED_ROW_SQL})\n"
        "SELECT 1\n"
    ),
}


@pytest.mark.parametrize(
    "test_case",
    [
        EscapedQuoteScenarioTestCase(
            description="escaped quotes and trailing backslashes before every compile marker",
            scenario_name="order_notes__escapes",
            expected_compiled_fragments=(
                "'2025-01-01' AS start_date",
                "'web' AS channel",
                "UPPER('a\\'b') AS shouted",
                "'tag' AS tag",
                "'EU' AS region",
                _NOTE_SQL,
            ),
            expected_stdout_fragment="PASS=1  FAIL=0  ERROR=0  SKIP=0  TOTAL=1",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_snowflake_backslash_escapes_when_compiling_and_replaying_locally_then_passes(
    test_case: EscapedQuoteScenarioTestCase, tmp_path: Path
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path, project_name="escaped_quotes", repo_files=_PROJECT_FILES
    )
    write_snowflake_order_ids_snapshot(
        project_dir=project_dir, scenario_name=test_case.scenario_name
    )

    compiled: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "compile"), project_dir=project_dir
    )
    replayed: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "scenario", "test", "--local", "--strict", test_case.scenario_name),
        project_dir=project_dir,
    )

    assert compiled.returncode == 0, compiled.stdout + compiled.stderr
    compiled_sql: str = (
        project_dir / "target" / "compiled" / "models" / "order_notes.sql"
    ).read_text()
    for fragment in test_case.expected_compiled_fragments:
        assert fragment in compiled_sql, compiled_sql
    assert replayed.returncode == 0, replayed.stdout + replayed.stderr
    assert test_case.expected_stdout_fragment in replayed.stdout, replayed.stdout


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
