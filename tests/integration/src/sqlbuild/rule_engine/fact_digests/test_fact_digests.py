"""Custom-rule fact digests are stable and change whenever the fact's inputs change."""

from pathlib import Path

import pytest

from tests.integration.src.sqlbuild.rule_engine.fact_digests._test_types import (
    FactDigestEditTestCase,
)
from tests.integration.src.sqlbuild.rule_engine.fact_digests.helpers import (
    PROJECT_FILES,
    fact_digest,
    write_files,
)

_STG: str = "sql/models/staging/stg_orders.sql"
_TOTALS: str = "sql/models/marts/order_totals.sql"
_NEW_MODEL: dict[str, str] = {
    "sql/models/marts/order_counts.sql": "MODEL ();\n\nSELECT 1 AS order_count\n"
}
_TOTALS_WITH_COLUMN: dict[str, str] = {
    _TOTALS: 'MODEL ();\n\nSELECT o.order_id, 1 AS amount FROM __ref("stg_orders") AS o\n'
}
_STG_TYPED: dict[str, str] = {
    _STG: (
        "MODEL (\n  columns (\n    order_id (type BIGINT, audits [not_null]),\n  ),\n);\n\n"
        'SELECT CAST(id AS INTEGER) AS order_id FROM __source("raw_orders")\n'
    )
}
_STG_AUDITED: dict[str, str] = {
    _STG: (
        "MODEL (\n  columns (\n    order_id (type INTEGER, audits [not_null, unique]),\n  ),\n);\n\n"
        'SELECT CAST(id AS INTEGER) AS order_id FROM __source("raw_orders")\n'
    )
}
_TOTALS_CONTRACT: dict[str, str] = {
    _TOTALS: (
        'MODEL (description "Order totals", contract enforced,\n'
        "  columns (order_id (type INTEGER)));\n\n"
        'SELECT o.order_id FROM __ref("stg_orders") AS o\n'
    )
}
_STG_ACCEPTED_INTEGERS: dict[str, str] = {
    _STG: (
        "MODEL (\n  columns (\n    order_id (type INTEGER, audits [accepted_values(values [1, 2])]),\n"
        "  ),\n);\n\n"
        'SELECT CAST(id AS INTEGER) AS order_id FROM __source("raw_orders")\n'
    )
}
_STG_ACCEPTED_STRINGS: dict[str, str] = {
    _STG: _STG_ACCEPTED_INTEGERS[_STG].replace("values [1, 2]", "values ['1', '2']")
}
_STG_STATUS: dict[str, str] = {
    _STG: (
        "MODEL (\n  columns (\n    order_id (type INTEGER),\n    status (type VARCHAR),\n  ),\n);\n\n"
        "SELECT CAST(id AS INTEGER) AS order_id, 'PAID' AS status FROM __source(\"raw_orders\")\n"
    )
}
_STG_STATUS_ENUM: dict[str, str] = {
    _STG: _STG_STATUS[_STG].replace("status (type VARCHAR)", "status (type order_status)")
}
_TOTALS_WITHOUT_REF: dict[str, str] = {_TOTALS: "MODEL ();\n\nSELECT 1 AS order_id\n"}
_TEST_EDITED: dict[str, str] = {
    "tests/unit/test_order_totals.sql": (
        "TEST ();\n\nWITH\n__ref__stg_orders AS (SELECT 2 AS order_id),\n"
        "__expected__order_totals AS (SELECT 2 AS order_id)\nSELECT 1\n"
    )
}


@pytest.mark.parametrize(
    "test_case",
    (
        FactDigestEditTestCase("project models", ("project.models",), _NEW_MODEL),
        FactDigestEditTestCase(
            "project sources",
            ("project.sources",),
            {"sql/sources/raw.yml": PROJECT_FILES["sql/sources/raw.yml"].replace("Raw", "All")},
        ),
        FactDigestEditTestCase(
            "project seeds",
            ("project.seeds",),
            {
                "sql/seeds/regions.yml": PROJECT_FILES["sql/seeds/regions.yml"].replace(
                    "Regions", "Areas"
                )
            },
        ),
        FactDigestEditTestCase(
            "project functions",
            ("project.functions",),
            {
                "sql/functions/sql/is_paid.sql": PROJECT_FILES[
                    "sql/functions/sql/is_paid.sql"
                ].replace("'PAID'", "'OPEN'")
            },
        ),
        FactDigestEditTestCase("project tests", ("project.tests",), _TEST_EDITED),
        FactDigestEditTestCase("all tests", ("tests.all",), _TEST_EDITED),
        FactDigestEditTestCase("project audits", ("project.audits",), _STG_AUDITED),
        FactDigestEditTestCase("all audits", ("audits.all",), _STG_AUDITED),
        FactDigestEditTestCase("tree paths", ("tree.paths",), {"config/notes.yaml": "a: 1\n"}),
        FactDigestEditTestCase(
            "public enums",
            ("declarations.public_enums",),
            {
                "sql/enums/order_status.sql": "ENUM (name order_status, members [PAID, OPEN, VOID]);\n"
            },
        ),
        FactDigestEditTestCase(
            "all enums",
            ("declarations.enums",),
            {"sql/enums/order_status.sql": "ENUM (name order_status, members [PAID]);\n"},
        ),
        FactDigestEditTestCase(
            "public constants",
            ("declarations.public_constants",),
            {"sql/constants/policy.sql": "CONSTANT (name minimum_amount, value 8);\n"},
        ),
        FactDigestEditTestCase(
            "all constants",
            ("declarations.constants",),
            {"sql/constants/policy.sql": "CONSTANT (name minimum_amount, value 9);\n"},
        ),
        FactDigestEditTestCase("model SQL", ("sql.for_model", _TOTALS), _TOTALS_WITH_COLUMN),
        FactDigestEditTestCase(
            "model dependencies", ("graph.dependencies", _TOTALS), _TOTALS_WITHOUT_REF
        ),
        FactDigestEditTestCase("model dependents", ("graph.dependents", _STG), _TOTALS_WITHOUT_REF),
        FactDigestEditTestCase("declared columns", ("columns.declared", _STG), _STG_TYPED),
        FactDigestEditTestCase(
            "inferred columns", ("columns.inferred", _TOTALS), _TOTALS_WITH_COLUMN
        ),
        FactDigestEditTestCase("column names", ("columns.names", _TOTALS), _TOTALS_WITH_COLUMN),
        FactDigestEditTestCase(
            "enum column types",
            ("columns.enum_types", _STG),
            _STG_STATUS_ENUM,
            base_files=_STG_STATUS,
        ),
        FactDigestEditTestCase("contract", ("contracts.enforced", _TOTALS), _TOTALS_CONTRACT),
        FactDigestEditTestCase("model tests", ("tests.for_model", "order_totals"), _TEST_EDITED),
        FactDigestEditTestCase("model audits", ("audits.for_model", "stg_orders"), _STG_AUDITED),
        FactDigestEditTestCase(
            "audit argument numbers become strings",
            ("columns.declared", _STG),
            _STG_ACCEPTED_STRINGS,
            base_files=_STG_ACCEPTED_INTEGERS,
        ),
        FactDigestEditTestCase("tree children", ("tree.children", "sql/models/marts"), _NEW_MODEL),
        FactDigestEditTestCase("tree descendants", ("tree.descendants", "sql/models"), _NEW_MODEL),
        FactDigestEditTestCase("tree glob", ("tree.glob", "sql/models/**/*.sql"), _NEW_MODEL),
        FactDigestEditTestCase(
            "resources under", ("tree.resources_under", "sql/models/marts"), _NEW_MODEL
        ),
        FactDigestEditTestCase(
            "read text", ("tree.read_text", "config/owners.yaml"), {"config/owners.yaml": "a: 2\n"}
        ),
        FactDigestEditTestCase(
            "missing model appears",
            ("sql.for_model", "sql/models/marts/order_counts.sql"),
            _NEW_MODEL,
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_fact_inputs_when_digesting_then_digest_is_stable_and_follows_edits(
    tmp_path: Path, test_case: FactDigestEditTestCase
) -> None:
    write_files(root=tmp_path, files={**PROJECT_FILES, **test_case.base_files})
    first: str | None = fact_digest(root=tmp_path, key=test_case.key)
    again: str | None = fact_digest(root=tmp_path, key=test_case.key)
    write_files(root=tmp_path, files=test_case.edited_files)
    edited: str | None = fact_digest(root=tmp_path, key=test_case.key)

    assert first is not None
    assert again == first
    assert edited is not None
    assert (edited != first) is test_case.expected_digest_changes


if __name__ == "__main__":
    raise SystemExit(pytest.main([__file__, "-n", "auto", "--dist", "loadfile"]))
