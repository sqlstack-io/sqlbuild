from __future__ import annotations

import subprocess
from pathlib import Path

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.sqlite._test_types import (
    SqliteDecimalContractE2ETestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.main.sqlite.helpers import (
    CHANGED_RAW_PAYMENTS_SQL,
    DECIMAL_PROJECT_FILES,
    EVOLVED_DECIMAL_PROJECT_FILES,
    EVOLVED_PAYMENT_ROWS_SQL,
    FEE_DECLARATION_SQL,
    LEDGER_AGREEMENT_SQL,
    PAYMENT_ROWS_SQL,
    RAW_PAYMENTS_SQL,
    SQLITE_FILE,
    UNDECLARED_PAYMENT_SOURCES,
    execute_sqlite_script,
    sqlite_rows,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import prepare_inline_project, run_sqb

_EXPECTED_PAYMENT_ROWS: tuple[tuple[object, ...], ...] = (
    (1, "1.50", "text"),
    (2, "2.00", "text"),
)
_EXPECTED_EVOLVED_ROWS: tuple[tuple[object, ...], ...] = (
    (1, "2.25", "0.10", "text"),
    (2, "2.00", "0.10", "text"),
    (3, "4.00", "0.10", "text"),
)


@pytest.mark.parametrize(
    "test_case",
    [
        SqliteDecimalContractE2ETestCase(
            description="a source declaring DECIMAL columns feeds canonical contract text",
            source_files={},
            expected_payment_rows=_EXPECTED_PAYMENT_ROWS,
            expected_agreeing_rows=2,
            expected_evolved_rows=_EXPECTED_EVOLVED_ROWS,
            expected_fee_declaration="TEXT DECIMAL(10,2)",
        ),
        SqliteDecimalContractE2ETestCase(
            description="a numeric DECIMAL(10,2) warehouse column is enforced to contract text",
            source_files={"sql/sources/raw.yml": UNDECLARED_PAYMENT_SOURCES},
            expected_payment_rows=_EXPECTED_PAYMENT_ROWS,
            expected_agreeing_rows=2,
            expected_evolved_rows=_EXPECTED_EVOLVED_ROWS,
            expected_fee_declaration="TEXT DECIMAL(10,2)",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_numeric_source_when_building_contract_models_then_sqlite_stores_decimal_text(
    tmp_path: Path, test_case: SqliteDecimalContractE2ETestCase
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="payments_shop",
        repo_files={**DECIMAL_PROJECT_FILES, **test_case.source_files},
    )
    db_path: Path = project_dir / SQLITE_FILE
    execute_sqlite_script(db_path=db_path, sql=RAW_PAYMENTS_SQL)

    first: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build"), project_dir=project_dir
    )
    payment_rows: tuple[tuple[object, ...], ...] = sqlite_rows(
        db_path=db_path, sql=PAYMENT_ROWS_SQL
    )
    agreeing: tuple[tuple[object, ...], ...] = sqlite_rows(
        db_path=db_path, sql=LEDGER_AGREEMENT_SQL
    )
    execute_sqlite_script(db_path=db_path, sql=CHANGED_RAW_PAYMENTS_SQL)
    prepare_inline_project(
        tmp_path=tmp_path, project_name="payments_shop", repo_files=EVOLVED_DECIMAL_PROJECT_FILES
    )
    second: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build", "--select", "payments"), project_dir=project_dir
    )

    assert (first.returncode, second.returncode) == (0, 0), (
        first.stdout + first.stderr + second.stdout + second.stderr
    )
    assert payment_rows == test_case.expected_payment_rows
    assert agreeing == ((test_case.expected_agreeing_rows,),)
    assert sqlite_rows(db_path=db_path, sql=EVOLVED_PAYMENT_ROWS_SQL) == (
        test_case.expected_evolved_rows
    )
    assert sqlite_rows(db_path=db_path, sql=FEE_DECLARATION_SQL) == (
        (test_case.expected_fee_declaration,),
    )
