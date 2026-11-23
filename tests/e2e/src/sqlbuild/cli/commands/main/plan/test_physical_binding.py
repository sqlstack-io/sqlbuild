"""Physical source closure must reach downstream models before execution."""

import subprocess
from pathlib import Path

import duckdb
import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.plan._test_types import RemovedInterfaceTestCase
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import run_sqb


@pytest.mark.parametrize(
    "test_case",
    [
        RemovedInterfaceTestCase("plan rejects downstream missing field", ("plan",), "B002", 1),
        RemovedInterfaceTestCase("build rejects before creating models", ("build",), "B002", 1),
    ],
    ids=lambda case: case.description,
)
def test_given_physical_source_shape_when_rebinding_then_rejects_downstream_before_mutation(
    test_case: RemovedInterfaceTestCase,
    tmp_path: Path,
) -> None:
    database: Path = tmp_path / "orders.duckdb"
    with duckdb.connect(str(database)) as connection:
        connection.execute("CREATE TABLE raw_orders (id INTEGER)")
    (tmp_path / "sqlbuild_project.toml").write_text(
        f'name = "orders"\nadapter = "duckdb"\n[connection]\ndatabase = "{database}"\n[rules]\nselect = []\n'
    )
    (tmp_path / "sql" / "sources").mkdir()
    (tmp_path / "sql/sources/orders.yml").write_text(
        "sources:\n  - name: raw_orders\n    description: Test source raw_orders.\n    table: raw_orders\n    schema: main\n"
    )
    (tmp_path / "sql" / "models").mkdir()
    (tmp_path / "sql/models/orders.sql").write_text(
        'MODEL (description "Test model orders.", materialized table, schema analytics); SELECT * FROM __source("raw_orders")'
    )
    (tmp_path / "sql/models/report.sql").write_text(
        'MODEL (description "Test model report.", materialized table, schema analytics); SELECT missing FROM __ref("orders")'
    )
    result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.command, project_dir=tmp_path
    )
    assert result.returncode == test_case.expected_exit_code
    assert test_case.expected_error in result.stdout + result.stderr
    with duckdb.connect(str(database)) as connection:
        assert (
            connection.execute(
                "SELECT table_name FROM information_schema.tables WHERE table_schema = 'analytics'"
            ).fetchall()
            == []
        )


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
