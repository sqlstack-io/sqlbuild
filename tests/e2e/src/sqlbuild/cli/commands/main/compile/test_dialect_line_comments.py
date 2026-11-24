"""A dialect's own line comments may hold quotes and reference text through the real CLI."""

from __future__ import annotations

import json
import subprocess
from pathlib import Path
from typing import Any

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.compile._test_types import (
    DialectLineCommentTestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import prepare_inline_project, run_sqb

_SNOWFLAKE_TOML: str = (
    'name = "orders"\nadapter = "snowflake"\n\n[connection]\naccount = "example"\n'
    'user = "analyst"\npassword = "unused"\ndatabase = "ANALYTICS"\nwarehouse = "COMPUTE"\n'
    'schema = "ORDERS"\n\n[defaults]\ndatabase = "ANALYTICS"\n'
)
_BIGQUERY_TOML: str = (
    'name = "orders"\nadapter = "bigquery"\n\n[connection]\nproject = "example-project"\n'
    'schema = "orders"\n\n[defaults]\ndatabase = "example-project"\n'
)
_BASE_ORDERS_SQL: str = "MODEL (description 'Orders.', materialized table);\n\nSELECT 1 AS order_id, 2 AS amount_cents\n"


@pytest.mark.parametrize(
    "test_case",
    [
        DialectLineCommentTestCase(
            description="snowflake double-slash comments with quotes, brackets and references",
            project_toml=_SNOWFLAKE_TOML,
            query_sql=(
                '// it\'s the order key, "quoted" and items[0]\n'
                'SELECT order_id, amount_cents // don\'t read __ref("missing_orders")\n'
                'FROM __ref("base_orders")'
            ),
            expected_column_count=2,
        ),
        DialectLineCommentTestCase(
            description="bigquery hash comments with quotes and references",
            project_toml=_BIGQUERY_TOML,
            query_sql=(
                '# it\'s the order key and "quoted"\n'
                'SELECT order_id, amount_cents # don\'t read __ref("missing_orders")\n'
                'FROM __ref("base_orders")'
            ),
            expected_column_count=2,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_dialect_line_comment_with_quotes_when_compiling_then_model_is_analysed(
    test_case: DialectLineCommentTestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="orders_project",
        repo_files={
            "sqlbuild_project.toml": test_case.project_toml,
            "sql/models/base_orders.sql": _BASE_ORDERS_SQL,
            "sql/models/commented_orders.sql": (
                "MODEL (description 'Commented orders.', materialized table);\n\n"
                f"{test_case.query_sql}\n"
            ),
        },
    )

    compiled: subprocess.CompletedProcess[str] = run_sqb(
        project_dir=project_dir, command=("compile", "--no-cache", "--json")
    )
    payload: dict[str, Any] = json.loads(compiled.stdout)
    column_counts: dict[str, int] = {
        model["name"]: model["column_count"] for model in payload["resources"]["models"]
    }

    assert compiled.returncode == 0, f"{test_case.description}: {compiled.stdout}{compiled.stderr}"
    assert payload["diagnostics"] == [], test_case.description
    assert column_counts["commented_orders"] == test_case.expected_column_count, (
        test_case.description
    )


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
