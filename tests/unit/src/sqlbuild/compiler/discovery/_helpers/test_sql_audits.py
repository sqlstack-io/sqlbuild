from __future__ import annotations

from pathlib import Path
from typing import cast

import pytest

from sqlbuild._native import DiscoveredAuditBlock, DiscoveredAuditFile, SqlAuditParseError
from sqlbuild.compiler.auditing.types import AuditEvaluationMode
from tests.unit.src.sqlbuild.compiler.discovery._helpers._test_types import (
    ParseMeasurementAuditTestCase,
    ParseSqlAuditFileErrorTestCase,
    ParseSqlAuditFileTestCase,
)
from tests.unit.src.sqlbuild.compiler.discovery._helpers.helpers import discover_declaration_file


@pytest.mark.parametrize(
    "test_case",
    [
        ParseSqlAuditFileTestCase(
            description="discovers one unnamed audit block",
            contents="""
        AUDIT ();

        SELECT order_id
        FROM __ref("orders")
        WHERE order_total < 0
        """,
            expected_names=(None,),
            expected_sql_bodies=('SELECT order_id\nFROM __ref("orders")\nWHERE order_total < 0',),
            expected_audit_indexes=(1,),
            expected_header_values=({},),
        ),
        ParseSqlAuditFileTestCase(
            description="discovers multiple named audit blocks from one file",
            contents="""
        AUDIT (name "negative totals");

        SELECT order_id
        FROM __ref("orders")
        WHERE order_total < 0;

        AUDIT (name "missing customers");

        SELECT customer_id
        FROM __ref("orders")
        WHERE customer_id IS NULL
        """,
            expected_names=("negative totals", "missing customers"),
            expected_sql_bodies=(
                'SELECT order_id\nFROM __ref("orders")\nWHERE order_total < 0;',
                'SELECT customer_id\nFROM __ref("orders")\nWHERE customer_id IS NULL',
            ),
            expected_audit_indexes=(1, 2),
            expected_header_values=(
                {"name": "negative totals"},
                {"name": "missing customers"},
            ),
        ),
        ParseSqlAuditFileTestCase(
            description="parses all supported audit fields",
            contents="""
        AUDIT (
          name "negative: totals",
          severity warn,
          run_scope delta_and_final,
          always_run true
        );

        SELECT 1
        """,
            expected_names=("negative: totals",),
            expected_sql_bodies=("SELECT 1",),
            expected_audit_indexes=(1,),
            expected_header_values=(
                {
                    "name": "negative: totals",
                    "severity": "warn",
                    "run_scope": "delta_and_final",
                    "always_run": True,
                },
            ),
        ),
        ParseSqlAuditFileTestCase(
            description="treats measurement function calls in select as violations SQL",
            contents="AUDIT (); SELECT measure(x), evidence(y) FROM t",
            expected_names=(None,),
            expected_sql_bodies=("SELECT measure(x), evidence(y) FROM t",),
            expected_audit_indexes=(1,),
            expected_header_values=({},),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_sql_audit_file_variants_when_parsing_then_it_returns_expected_raw_blocks(
    test_case: ParseSqlAuditFileTestCase,
    tmp_path: Path,
) -> None:
    discovered_blocks: tuple[DiscoveredAuditBlock, ...] = cast(
        DiscoveredAuditFile,
        discover_declaration_file(
            project_dir=tmp_path,
            relative_path="sql/audits/generic/orders.sql",
            contents=test_case.contents,
        ),
    ).blocks

    assert tuple(block.name for block in discovered_blocks) == test_case.expected_names
    assert tuple(block.sql_body for block in discovered_blocks) == test_case.expected_sql_bodies
    assert (
        tuple(block.audit_index for block in discovered_blocks) == test_case.expected_audit_indexes
    )
    assert tuple(block.header_values for block in discovered_blocks) == (
        test_case.expected_header_values
    )


@pytest.mark.parametrize(
    "test_case",
    [
        ParseSqlAuditFileErrorTestCase(
            description="raises when the file does not start with an audit header",
            contents="SELECT 1\n",
            expected_error_fragment="must start with an AUDIT",
        ),
        ParseSqlAuditFileErrorTestCase(
            description="raises when leading comments appear before the first audit header",
            contents="-- comment\nAUDIT ();\n\nSELECT 1\n",
            expected_error_fragment="must start with an AUDIT",
        ),
        ParseSqlAuditFileErrorTestCase(
            description="raises when an audit block has no sql body",
            contents="AUDIT ();\n",
            expected_error_fragment="must define SQL after AUDIT(...)",
        ),
        ParseSqlAuditFileErrorTestCase(
            description="rejects the old colon syntax",
            contents="""
        AUDIT (name: "legacy");

        SELECT 1
        """,
            expected_error_fragment="unexpected ':' after key 'name'",
        ),
        ParseSqlAuditFileErrorTestCase(
            description="raises when the audit header includes unsupported keys",
            contents="""
        AUDIT (name "negative totals", unsupported true);

        SELECT 1
        """,
            expected_error_fragment="unsupported keys: unsupported",
        ),
        ParseSqlAuditFileErrorTestCase(
            description="raises when the audit name is blank",
            contents="""
        AUDIT (name "   ");

        SELECT 1
        """,
            expected_error_fragment="must be a non-empty string",
        ),
        ParseSqlAuditFileErrorTestCase(
            description="raises when the audit name is not a string",
            contents="""
        AUDIT (name 123);

        SELECT 1
        """,
            expected_error_fragment="must be a non-empty string",
        ),
        ParseSqlAuditFileErrorTestCase(
            description="rejects an explicit null audit name",
            contents="AUDIT (name null);\n\nSELECT 1\n",
            expected_error_fragment="name.*must be a non-empty string",
        ),
        ParseSqlAuditFileErrorTestCase(
            description="rejects an explicit null audit severity",
            contents="AUDIT (severity null);\n\nSELECT 1\n",
            expected_error_fragment="severity.*must be a non-empty string",
        ),
        ParseSqlAuditFileErrorTestCase(
            description="rejects an explicit null audit run scope",
            contents="AUDIT (run_scope null);\n\nSELECT 1\n",
            expected_error_fragment="run_scope.*must be a non-empty string",
        ),
        ParseSqlAuditFileErrorTestCase(
            description="rejects an explicit null audit always run flag",
            contents="AUDIT (always_run null);\n\nSELECT 1\n",
            expected_error_fragment="always_run.*must be a boolean",
        ),
        ParseSqlAuditFileErrorTestCase(
            description="raises when a multi-block file leaves one audit unnamed",
            contents="""
        AUDIT (name "negative totals");

        SELECT 1;

        AUDIT ();

        SELECT 1
        """,
            expected_error_fragment="every block must define a unique `name`",
        ),
        ParseSqlAuditFileErrorTestCase(
            description="raises when a multi-block file repeats an audit name",
            contents="""
        AUDIT (name "shared");

        SELECT 1;

        AUDIT (name "shared");

        SELECT 1
        """,
            expected_error_fragment=r"defines duplicate AUDIT\(\) name 'shared'",
        ),
        ParseSqlAuditFileErrorTestCase(
            description="rejects duplicate audit keys",
            contents='AUDIT (name "first", name "second");\n\nSELECT 1\n',
            expected_error_fragment="duplicate.*name",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_invalid_sql_audit_file_contents_when_parsing_then_it_raises_clear_errors(
    test_case: ParseSqlAuditFileErrorTestCase,
    tmp_path: Path,
) -> None:
    with pytest.raises(SqlAuditParseError, match=test_case.expected_error_fragment):
        discover_declaration_file(
            project_dir=tmp_path,
            relative_path="sql/audits/generic/orders.sql",
            contents=test_case.contents,
        )


@pytest.mark.parametrize(
    "test_case",
    [
        ParseMeasurementAuditTestCase(
            description="measurement contract and parenthesized SQL strings",
            contents="""
                AUDIT (
                  evaluation measurement,
                  value valid_rate,
                  sample_count total_rows,
                  sample_unit "input rows"
                );
                MEASURE (
                  SELECT COUNT(*) AS total_rows, 'rate (valid)' AS label, 99.5 AS valid_rate
                );
                EVIDENCE (
                  SELECT * FROM orders WHERE note = 'unexpected ) value'
                );
            """,
            expected_value_column="valid_rate",
            expected_sample_count_column="total_rows",
            expected_sample_unit="input rows",
            expected_measure_fragment="rate (valid)",
            expected_evidence_fragment="unexpected ) value",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_measurement_audit_with_evidence_when_parsing_then_contract_and_queries_are_separate(
    test_case: ParseMeasurementAuditTestCase,
    tmp_path: Path,
) -> None:
    blocks: tuple[DiscoveredAuditBlock, ...] = cast(
        DiscoveredAuditFile,
        discover_declaration_file(
            project_dir=tmp_path,
            relative_path="sql/audits/generic/rate.sql",
            contents=test_case.contents,
        ),
    ).blocks

    block: DiscoveredAuditBlock = blocks[0]
    assert block.evaluation_mode == AuditEvaluationMode.MEASUREMENT
    assert block.measurement_contract is not None
    assert block.measurement_contract.value_column == test_case.expected_value_column
    assert block.measurement_contract.sample_count_column == test_case.expected_sample_count_column
    assert block.measurement_contract.sample_unit == test_case.expected_sample_unit
    assert test_case.expected_measure_fragment in cast(str, block.measure_sql)
    assert test_case.expected_evidence_fragment in cast(str, block.evidence_sql)


@pytest.mark.parametrize(
    "test_case",
    [
        ParseSqlAuditFileErrorTestCase(
            description="measurement missing value",
            contents="AUDIT (evaluation measurement); MEASURE (SELECT 1);",
            expected_error_fragment="must define `value`",
        ),
        ParseSqlAuditFileErrorTestCase(
            description="measurement has evidence only",
            contents="AUDIT (evaluation measurement, value rate); EVIDENCE (SELECT 1);",
            expected_error_fragment=r"must define exactly one MEASURE\(\.\.\.\) block",
        ),
        ParseSqlAuditFileErrorTestCase(
            description="measurement has bare body",
            contents="AUDIT (evaluation measurement, value rate); SELECT 1;",
            expected_error_fragment=r"must define exactly one MEASURE\(\.\.\.\) block",
        ),
        ParseSqlAuditFileErrorTestCase(
            description="violations audit has measure block",
            contents="AUDIT (); MEASURE (SELECT 1);",
            expected_error_fragment="must use a bare SELECT body",
        ),
        ParseSqlAuditFileErrorTestCase(
            description="violations audit has measurement contract",
            contents="AUDIT (value rate); SELECT 1;",
            expected_error_fragment="must not define measurement keys",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_invalid_evaluation_grammar_when_parsing_then_clear_error_is_raised(
    test_case: ParseSqlAuditFileErrorTestCase,
    tmp_path: Path,
) -> None:
    with pytest.raises(SqlAuditParseError, match=test_case.expected_error_fragment):
        discover_declaration_file(
            project_dir=tmp_path,
            relative_path="sql/audits/generic/rate.sql",
            contents=test_case.contents,
        )
