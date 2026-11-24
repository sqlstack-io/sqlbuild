"""Real-CLI coverage of per-batch microbatch progress when stdout is not a terminal."""

from __future__ import annotations

import re
import subprocess
from pathlib import Path
from textwrap import dedent

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.build._test_types import (
    MicrobatchBatchProgressE2ETestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import (
    execute_duckdb,
    prepare_inline_project,
    run_sqb,
    run_sqb_with_pty,
)

_DATABASE: str = "warehouse.duckdb"
_PROJECT_FILES: dict[str, str] = {
    "sqlbuild_project.toml": f'name = "batch_progress"\nadapter = "duckdb"\n\n[connection]\ndatabase = "{_DATABASE}"\n',
    "sql/sources/raw.yml": (
        "sources:\n  - name: raw_orders\n    description: Test source raw_orders.\n"
        "    schema: main\n    table: raw_orders\n"
    ),
    "sql/models/daily_orders.sql": dedent(
        """
        MODEL (description "Test model daily_orders.",
          materialized incremental,
          incremental_strategy delete_insert,
          incremental_mode microbatch,
          microbatch_strategy watermark,
          cursor_watermark_mode all,
          cursor order_date,
          cursor_type timestamp,
          cursor_grain day,
          cursor_start '2026-07-01',
          cursor_inputs (
            raw_orders (column order_date, roles [filter, watermark]),
          ),
          batch_size 1d,
        );
        SELECT order_id, order_date FROM __source("raw_orders")
        """
    ).strip()
    + "\n",
}
_RAW_ORDERS: str = (
    "CREATE TABLE raw_orders (order_id INTEGER, order_date TIMESTAMP);"
    "INSERT INTO raw_orders VALUES (1, '2026-07-01 08:00:00'), (2, '2026-07-01 09:00:00'),"
    " (3, '2026-07-02 10:00:00'), (4, '2026-07-03 11:00:00');"
)
_BATCH_LINE: re.Pattern[str] = re.compile(
    r"^Batch  daily_orders  (\d+)/(\d+)  \S+  (\d+) rows  \d+\.\ds$", re.MULTILINE
)


@pytest.mark.parametrize(
    "test_case",
    [
        MicrobatchBatchProgressE2ETestCase(
            description="captured stdout keeps one line per batch with its rows",
            run=run_sqb,
            expected_batches=(("1", "3", "2"), ("2", "3", "1"), ("3", "3", "1")),
        ),
        MicrobatchBatchProgressE2ETestCase(
            description="a terminal keeps batch progress on the live row only",
            run=run_sqb_with_pty,
            expected_batches=(),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_multi_batch_microbatch_when_building_then_batch_lines_follow_the_output_kind(
    test_case: MicrobatchBatchProgressE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path, project_name="batch_progress", repo_files=_PROJECT_FILES
    )
    execute_duckdb(db_path=project_dir / _DATABASE, sql=_RAW_ORDERS)

    result: subprocess.CompletedProcess[str] = test_case.run(
        command=("--no-color", "build"), project_dir=project_dir
    )

    output: str = result.stdout.replace("\r\n", "\n")
    assert result.returncode == 0, output + result.stderr
    assert tuple(_BATCH_LINE.findall(output)) == test_case.expected_batches, output


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
