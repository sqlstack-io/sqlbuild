from pathlib import Path

import pytest

from sqlbuild.adapter.contract._helpers.raw_statement_audit import audit_raw_statement_calls
from tests.unit.src.sqlbuild.adapter.contract._test_types import (
    RawStatementAuditCase,
    RawStatementAuditFixtureCase,
)


@pytest.mark.parametrize(
    "test_case",
    [
        RawStatementAuditCase(
            description="adapter and virtual state raw calls use approved boundaries",
            expected_violations=(),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_repository_when_auditing_raw_statements_then_only_approved_boundaries_remain(
    test_case: RawStatementAuditCase,
) -> None:
    source_root: Path = Path(__file__).parents[6] / "src" / "sqlbuild"

    violations: tuple[str, ...] = audit_raw_statement_calls(source_root=source_root)

    assert violations == test_case.expected_violations


@pytest.mark.parametrize(
    "test_case",
    [
        RawStatementAuditFixtureCase(
            description="nested raw attributes and aliases are violations",
            source_files=(
                (
                    "adapters/example.py",
                    "class Bad:\n"
                    "    def nested(self, connection):\n"
                    "        connection.raw_connection.execute('SELECT 1')\n"
                    "    def cursor_alias(self):\n"
                    "        cursor = self.raw_cursor\n"
                    "        cursor.executemany('INSERT', [])\n"
                    "    def client_alias(self):\n"
                    "        query_client = self.client\n"
                    "        query_client.query('SELECT 1')\n",
                ),
            ),
            expected_violations=(
                "adapters/example.py:3",
                "adapters/example.py:6",
                "adapters/example.py:9",
            ),
        ),
        RawStatementAuditFixtureCase(
            description="only exact low-level methods are approved",
            source_files=(
                (
                    "adapter/contract/classes/observed_connection.py",
                    "class ObservedConnection:\n"
                    "    def execute(self, sql):\n"
                    "        return self.raw_connection.execute(sql)\n"
                    "    def executemany(self, sql, rows):\n"
                    "        return self.raw_connection.executemany(sql, rows)\n",
                ),
                (
                    "adapter/contract/classes/observed_cursor.py",
                    "class ObservedCursor:\n"
                    "    def execute(self, sql):\n"
                    "        return self.raw_cursor.execute(sql)\n"
                    "    def executemany(self, sql, rows):\n"
                    "        return self.raw_cursor.executemany(sql, rows)\n",
                ),
                (
                    "adapters/bigquery/classes/bigquery_connection.py",
                    "class _BigQueryConnection:\n"
                    "    def query_job(self, sql):\n"
                    "        return self.client.query(sql)\n",
                ),
            ),
            expected_violations=(),
        ),
        RawStatementAuditFixtureCase(
            description="unapproved method in approved module is a violation",
            source_files=(
                (
                    "adapter/contract/classes/observed_connection.py",
                    "class ObservedConnection:\n"
                    "    def bypass(self, sql):\n"
                    "        return self.raw_connection.execute(sql)\n",
                ),
            ),
            expected_violations=("adapter/contract/classes/observed_connection.py:3",),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_source_fixture_when_auditing_raw_statements_then_expected_calls_are_reported(
    test_case: RawStatementAuditFixtureCase,
    tmp_path: Path,
) -> None:
    source_root: Path = tmp_path / "sqlbuild"
    for relative_path, source in test_case.source_files:
        path: Path = source_root / relative_path
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(source, encoding="utf-8")

    violations: tuple[str, ...] = audit_raw_statement_calls(source_root=source_root)

    assert violations == test_case.expected_violations
