from decimal import Decimal
from pathlib import Path

import pytest

from sqlbuild._native import (
    ConstantDeclaration,
    declaration_rows,
    normalize_sql_value,
)
from tests.unit.src.sqlbuild.rule_engine._helpers.engine._test_types import (
    TypedConstantPayloadTestCase,
)


@pytest.mark.parametrize(
    "test_case",
    [
        TypedConstantPayloadTestCase(
            description="nested typed value is JSON-safe and type-bearing",
            raw_value={"rate": Decimal("2.4700"), "countries": ["GB", "FR"]},
            expected_value={"countries": ["GB", "FR"], "rate": "2.4700"},
            expected_type="object",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_typed_constant_when_building_native_row_then_value_and_type_are_preserved(
    test_case: TypedConstantPayloadTestCase,
) -> None:
    declaration: ConstantDeclaration = ConstantDeclaration(
        name="rules",
        value=normalize_sql_value(raw_value=test_case.raw_value, context="constant 'rules'"),
        relative_path=Path("sql/constants/rules.sql"),
    )

    _, _, members, value, value_type, render_as = declaration_rows(
        enums=(), constants=(declaration,)
    )[1][0]

    assert members == []
    assert value == test_case.expected_value
    assert value_type == test_case.expected_type
    assert render_as is None


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
